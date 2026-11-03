use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use image::RgbaImage;

#[derive(Clone, Debug)]
pub struct UpscaleOptions {
    pub model: String,
    pub gpu_id: i32,
    pub tile_size: u32,
}

impl Default for UpscaleOptions {
    fn default() -> Self {
        Self { model: "realesrgan-x4plus".into(), gpu_id: -1, tile_size: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct VulkanGpu {
    pub id: i32,
    pub name: String,
    pub discrete: bool,
}

/// Enumerate Vulkan physical devices in Vulkan's native order.
/// This matches the GPU indices used by Real-ESRGAN ncnn-vulkan.
pub fn available_vulkan_gpus() -> Result<Vec<VulkanGpu>, String> {
    use ash::vk;
    use std::ffi::CStr;

    let entry = unsafe { ash::Entry::load() }
        .map_err(|e| format!("Vulkan loader unavailable: {e}"))?;
    let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_0);
    let create_info = vk::InstanceCreateInfo::default().application_info(&app);
    let instance = unsafe { entry.create_instance(&create_info, None) }
        .map_err(|e| format!("Cannot initialize Vulkan: {e}"))?;
    let devices = unsafe { instance.enumerate_physical_devices() };
    let result = devices.map(|devices| {
        devices.iter().enumerate().map(|(id, device)| {
            let properties = unsafe { instance.get_physical_device_properties(*device) };
            let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
                .to_string_lossy().into_owned();
            VulkanGpu { id: id as i32, name, discrete: properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU }
        }).collect()
    });
    unsafe { instance.destroy_instance(None); }
    result.map_err(|e| format!("Cannot enumerate Vulkan GPUs: {e}"))
}

/// Prefer a discrete Vulkan GPU; preserve native enumeration order as tie-breaker.
pub fn preferred_gpu_id(gpus: &[VulkanGpu]) -> Option<i32> {
    gpus.iter().find(|gpu| gpu.discrete).or_else(|| gpus.first()).map(|gpu| gpu.id)
}

pub struct UpscaleResult {
    pub work_dir: PathBuf,
    pub ai_path: PathBuf,
    pub baseline_path: PathBuf,
    pub preview: RgbaImage,
    pub baseline_preview: RgbaImage,
    pub dimensions: [u32; 2],
}

impl Drop for UpscaleResult {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.work_dir);
    }
}

pub fn engine_path() -> PathBuf {
    if let Some(folder)=crate::dependencies::active_path(crate::dependencies::Component::Upscale) {return folder.join("realesrgan-ncnn-vulkan.exe");}
    let bundled = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("tools").join("realesrgan").join("realesrgan-ncnn-vulkan.exe")));
    if let Some(path) = bundled {
        if path.is_file() { return path; }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("realesrgan")
        .join("realesrgan-ncnn-vulkan.exe")
}

fn expected_dimensions(width: u32, height: u32, scale: u32) -> Result<[u32; 2], String> {
    if scale != 2 && scale != 4 {
        return Err(format!("Unsupported AI scale: {scale}"));
    }

    let width = width.checked_mul(scale).ok_or("Upscaled width overflow")?;

    let height = height
        .checked_mul(scale)
        .ok_or("Upscaled height overflow")?;

    Ok([width, height])
}

fn validate_dimensions(image: &RgbaImage, expected: [u32; 2], label: &str) -> Result<(), String> {
    if image.width() != expected[0] || image.height() != expected[1] {
        return Err(format!(
            "{label}: expected {}x{}, got {}x{}",
            expected[0],
            expected[1],
            image.width(),
            image.height()
        ));
    }

    Ok(())
}

/// Check that requested and native inference dimensions are representable.
/// Memory pressure is managed by the inference engine and its tile-size setting.
pub fn validate_upscale_size(width: u32, height: u32, scale: u32) -> Result<(), String> {
    expected_dimensions(width, height, scale)?;
    expected_dimensions(width, height, 4)?;
    Ok(())
}

pub fn upscale(
    mut original: RgbaImage,
    settings: crate::adjustments::Adjustments,
    scale: u32,
    options: UpscaleOptions,
    cancel: Arc<AtomicBool>,
) -> Result<UpscaleResult, String> {
    validate_upscale_size(original.width(), original.height(), scale)?;
    let expected = expected_dimensions(original.width(), original.height(), scale)?;
    if !matches!(options.model.as_str(), "realesrgan-x4plus" | "realesrgan-x4plus-anime") { return Err("Unsupported upscale model".into()); }
    if options.gpu_id < -1 || options.gpu_id > 32 { return Err("Invalid GPU ID".into()); }
    if options.tile_size != 0 && !(32..=1024).contains(&options.tile_size) { return Err("Invalid tile size".into()); }

    // Validate the native 4x intermediate dimensions too.
    let _native_dimensions = expected_dimensions(original.width(), original.height(), 4)?;

    let engine = engine_path();

    if !engine.is_file() {
        return Err(format!(
            "Real-ESRGAN engine not found: {}",
            engine.display()
        ));
    }

    let model_dir = engine.parent().ok_or("Invalid engine path")?.join("models");
    let model_files = [model_dir.join(format!("{}.param", options.model)), model_dir.join(format!("{}.bin", options.model))];
    for model_file in model_files {
        if !model_file.is_file() { return Err(format!("Model file missing: {}", model_file.display())); }
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();

    let work = std::env::temp_dir().join(format!("pixelforge_ai_{}_{}", std::process::id(), stamp));

    std::fs::create_dir_all(&work).map_err(|e| format!("Cannot create AI work folder: {e}"))?;

    let result = upscale_inner(&mut original, &settings, scale, expected, &engine, &work, &options, &cancel);

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&work);
    }

    result
}

fn upscale_inner(
    original: &mut RgbaImage,
    settings: &crate::adjustments::Adjustments,
    scale: u32,
    expected: [u32; 2],
    engine: &Path,
    work: &Path,
    options: &UpscaleOptions,
    cancel: &AtomicBool,
) -> Result<UpscaleResult, String> {
    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    // Apply the existing PixelForge adjustments to the
    // full-resolution image before neural-network inference.
    for pixel in original.pixels_mut() {
        settings.apply_pixel(pixel);
    }

    let input = work.join("input.png");
    let output = work.join("native_4x.png");

    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    original
        .save(&input)
        .map_err(|e| format!("Cannot prepare AI input: {e}"))?;

    let engine_dir = engine.parent().ok_or("Invalid AI engine directory")?;

    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    // Drain stderr concurrently so Real-ESRGAN cannot block on a full pipe.
    // Keep a bounded diagnostic tail for reporting engine failures.
    let mut command = Command::new(engine);
    command.current_dir(engine_dir)
        .arg("-i").arg(&input)
        .arg("-o").arg(&output)
        .arg("-n").arg(&options.model)
        .arg("-m").arg(engine_dir.join("models"))
        .arg("-s").arg("4")
        .arg("-t").arg(options.tile_size.to_string());
    // ncnn's default GPU selection is used when no -g argument is supplied.
    // Passing -g -1 is not supported by all Real-ESRGAN builds.
    if options.gpu_id >= 0 {
        command.arg("-g").arg(options.gpu_id.to_string());
    }
    let mut process = command
        .arg("-f").arg("png")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot launch Real-ESRGAN: {e}"))?;

    let stderr = process.stderr.take().ok_or("Cannot capture Real-ESRGAN diagnostics")?;
    let diagnostics = std::thread::spawn(move || {
        let mut reader = stderr;
        let mut buffer = [0u8; 4096];
        let mut tail = Vec::new();
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 { break; }
            tail.extend_from_slice(&buffer[..count]);
            if tail.len() > 16_384 {
                tail.drain(..tail.len() - 16_384);
            }
        }
        String::from_utf8_lossy(&tail).into_owned()
    });

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = process.kill();
            let _ = process.wait();
            let _ = diagnostics.join();
            return Err("AI upscale cancelled".into());
        }
        match process.try_wait() {
            Ok(Some(status)) if status.success() => {
                let _ = diagnostics.join();
                break;
            }
            Ok(Some(status)) => {
                let details = diagnostics.join().unwrap_or_default();
                let details = details.trim();
                return Err(if details.is_empty() {
                    format!("Real-ESRGAN failed ({status}); no diagnostics returned")
                } else {
                    format!("Real-ESRGAN failed ({status}): {details}")
                });
            },
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                let _ = process.kill();
                let _ = process.wait();
                let _ = diagnostics.join();
                return Err(format!("Cannot monitor Real-ESRGAN: {e}"));
            }
        }
    }
    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }

    if !output.is_file() {
        return Err("Real-ESRGAN did not produce an output image".into());
    }

    // Allow large AI output PNGs while retaining a bounded
    // decoder allocation limit.
    let file = std::fs::File::open(&output).map_err(|e| format!("Cannot open AI output: {e}"))?;

    let reader = std::io::BufReader::new(file);

    let decoder = image::codecs::png::PngDecoder::new(reader)
        .map_err(|e| format!("Cannot initialize AI PNG decoder: {e}"))?;

    let mut limits = image::Limits::default();
    limits.max_alloc = Some(1024 * 1024 * 1024);

    let mut decoder = decoder;

    image::ImageDecoder::set_limits(&mut decoder, limits)
        .map_err(|e| format!("AI PNG exceeds memory limit: {e}"))?;

    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    let native = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Cannot decode AI output: {e}"))?
        .to_rgba8();

    let native_expected = expected_dimensions(original.width(), original.height(), 4)?;

    validate_dimensions(&native, native_expected, "Native 4x AI result")?;

    // For 2x, run genuine 4x AI inference first and
    // downsample with Lanczos3. This avoids the
    // problematic direct 2x inference path.
    let result = if scale == 2 {
        image::imageops::resize(
            &native,
            expected[0],
            expected[1],
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        native
    };

    validate_dimensions(&result, expected, "Final AI result")?;

    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    // Compare against a conventional high-quality resize.
    // Both paths use the same adjusted source pixels.
    let baseline = image::imageops::resize(
        original,
        expected[0],
        expected[1],
        image::imageops::FilterType::Lanczos3,
    );

    // Retain the two unblended full-resolution images.
    let ai_path = work.join("ai_result.png");
    let baseline_path = work.join("lanczos_result.png");

    result
        .save(&ai_path)
        .map_err(|e| format!("Cannot retain AI result: {e}"))?;

    baseline
        .save(&baseline_path)
        .map_err(|e| format!("Cannot retain Lanczos result: {e}"))?;

    let dimensions = [result.width(), result.height()];
    let longest = dimensions[0].max(dimensions[1]);

    let preview_size = if longest > 2048 {
        let ratio = 2048.0 / longest as f64;
        [
            ((dimensions[0] as f64 * ratio).round() as u32).max(1),
            ((dimensions[1] as f64 * ratio).round() as u32).max(1),
        ]
    } else {
        dimensions
    };

    let preview = image::imageops::resize(
        &result,
        preview_size[0],
        preview_size[1],
        image::imageops::FilterType::Triangle,
    );

    let baseline_preview = image::imageops::resize(
        &baseline,
        preview_size[0],
        preview_size[1],
        image::imageops::FilterType::Triangle,
    );

    if cancel.load(Ordering::Relaxed) { return Err("AI upscale cancelled".into()); }
    Ok(UpscaleResult {
        work_dir: work.to_path_buf(),
        ai_path,
        baseline_path,
        preview,
        baseline_preview,
        dimensions,
    })
}

pub fn blend_images(
    ai: &RgbaImage,
    baseline: &RgbaImage,
    strength: f32,
) -> Result<RgbaImage, String> {
    if ai.dimensions() != baseline.dimensions() {
        return Err("AI and Lanczos dimensions differ".into());
    }

    let strength = strength.clamp(0.0, 1.0);

    if strength >= 1.0 {
        return Ok(ai.clone());
    }

    if strength <= 0.0 {
        return Ok(baseline.clone());
    }

    let mut output = baseline.clone();

    for (out, neural) in output.pixels_mut().zip(ai.pixels()) {
        for channel in 0..4 {
            let a = out.0[channel] as f32;
            let b = neural.0[channel] as f32;
            out.0[channel] = (a + (b - a) * strength).round().clamp(0.0, 255.0) as u8;
        }
    }

    Ok(output)
}

/// Blend directly into an owned Lanczos image.
///
/// Unlike blend_images(), this does not allocate another
/// full-resolution output buffer.
pub fn blend_images_owned(
    ai: &RgbaImage,
    mut baseline: RgbaImage,
    strength: f32,
) -> Result<RgbaImage, String> {
    if ai.dimensions() != baseline.dimensions() {
        return Err("AI and Lanczos dimensions differ".into());
    }

    let strength = strength.clamp(0.0, 1.0);

    if strength <= 0.0 {
        return Ok(baseline);
    }

    if strength >= 1.0 {
        baseline.as_mut().copy_from_slice(ai.as_raw());
        return Ok(baseline);
    }

    // Process in bounded pixel batches. The output buffer
    // is reused rather than duplicated.
    const BATCH_PIXELS: usize = 65536;
    const CHANNELS: usize = 4;

    let batch_bytes = BATCH_PIXELS * CHANNELS;

    for (out_batch, ai_batch) in baseline
        .as_mut()
        .chunks_mut(batch_bytes)
        .zip(ai.as_raw().chunks(batch_bytes))
    {
        for (out, neural) in out_batch.iter_mut().zip(ai_batch.iter()) {
            let a = *out as f32;
            let b = *neural as f32;

            *out = (a + (b - a) * strength).round().clamp(0.0, 255.0) as u8;
        }
    }

    Ok(baseline)
}
/// Decode a large PNG for full-resolution export.
///
/// The default image::open() allocation limit is too small
/// for some Real-ESRGAN output images.
fn load_export_png(path: &Path, label: &str) -> Result<RgbaImage, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open {label}: {e}"))?;

    let reader = std::io::BufReader::new(file);

    let mut decoder = image::codecs::png::PngDecoder::new(reader)
        .map_err(|e| format!("Cannot initialize {label} decoder: {e}"))?;

    let mut limits = image::Limits::default();
    limits.max_alloc = Some(1024 * 1024 * 1024);

    image::ImageDecoder::set_limits(&mut decoder, limits)
        .map_err(|e| format!("{label} exceeds decoder memory limit: {e}"))?;

    image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Cannot decode {label}: {e}"))
        .map(|image| image.to_rgba8())
}
pub fn save_blended(
    ai_path: &Path,
    baseline_path: &Path,
    strength: f32,
    destination: &Path,
) -> Result<PathBuf, String> {
    if destination.exists() {
        return Err("Destination already exists".into());
    }

    let ai = load_export_png(ai_path, "AI result")?;

    let baseline = load_export_png(baseline_path, "Lanczos result")?;

    let output = blend_images_owned(&ai, baseline, strength)?;

    let filename = destination
        .file_name()
        .ok_or("Invalid destination filename")?
        .to_string_lossy();

    let temporary = destination.with_file_name(format!(".{filename}.pixelforge-partial.png"));

    if temporary.exists() {
        return Err("A partial export file already exists".into());
    }

    let save_result = (|| -> Result<(), String> {
        output
            .save_with_format(&temporary, image::ImageFormat::Png)
            .map_err(|e| format!("Cannot save PNG: {e}"))?;

        let dimensions =
            image::image_dimensions(&temporary).map_err(|e| format!("Cannot verify PNG: {e}"))?;

        if dimensions != output.dimensions() {
            return Err("Saved PNG dimensions are incorrect".into());
        }

        if destination.exists() {
            return Err("Destination appeared during export".into());
        }

        std::fs::rename(&temporary, destination)
            .map_err(|e| format!("Cannot finalize PNG: {e}"))?;

        Ok(())
    })();

    if save_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }

    save_result?;
    Ok(destination.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_image_preflight() {
        assert!(validate_upscale_size(1000, 1000, 4).is_ok());
        assert!(validate_upscale_size(10000, 10000, 2).is_ok());
        assert!(validate_upscale_size(u32::MAX, 10, 4).is_err());
    }

    #[test]
    fn owned_blending_matches_existing_blending() {
        let mut ai = RgbaImage::new(17, 13);
        let mut baseline = RgbaImage::new(17, 13);

        for (i, pixel) in ai.pixels_mut().enumerate() {
            *pixel = image::Rgba([(i % 256) as u8, 210, 37, 255]);
        }

        for (i, pixel) in baseline.pixels_mut().enumerate() {
            *pixel = image::Rgba([42, (i % 256) as u8, 190, 255]);
        }

        for strength in [0.0, 0.1, 0.5, 0.85, 1.0] {
            let expected = blend_images(&ai, &baseline, strength).unwrap();

            let actual = blend_images_owned(&ai, baseline.clone(), strength).unwrap();

            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn owned_blending_rejects_mismatched_dimensions() {
        let ai = RgbaImage::new(8, 8);
        let baseline = RgbaImage::new(7, 8);

        assert!(blend_images_owned(&ai, baseline, 0.5).is_err());
    }
    #[test]
    fn engine_path_is_predictable() {
        assert!(
            engine_path()
                .to_string_lossy()
                .contains("realesrgan-ncnn-vulkan.exe")
        );
    }

    #[test]
    fn two_x_dimensions_are_correct() {
        assert_eq!(expected_dimensions(2048, 2048, 2).unwrap(), [4096, 4096]);
    }

    #[test]
    fn four_x_dimensions_are_correct() {
        assert_eq!(expected_dimensions(2048, 2048, 4).unwrap(), [8192, 8192]);
    }

    #[test]
    fn invalid_scale_is_rejected() {
        assert!(expected_dimensions(10, 10, 3).is_err());
    }

    #[test]
    fn overflow_is_rejected() {
        assert!(expected_dimensions(u32::MAX, 10, 4).is_err());
    }

    #[test]
    fn correct_dimensions_pass() {
        let img = RgbaImage::new(16, 16);

        assert!(validate_dimensions(&img, [16, 16], "test").is_ok());
    }

    #[test]
    fn incorrect_dimensions_fail() {
        let img = RgbaImage::new(16, 16);

        assert!(validate_dimensions(&img, [32, 32], "test").is_err());
    }

    #[test]
    fn lanczos_downsample_has_correct_size() {
        let native = RgbaImage::new(40, 40);

        let reduced =
            image::imageops::resize(&native, 20, 20, image::imageops::FilterType::Lanczos3);

        assert_eq!(reduced.dimensions(), (20, 20));
    }
}
