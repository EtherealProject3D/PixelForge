//! Adapter for dedicated neural sharpening and deblurring executables.
//! Each external engine must accept --input <png> --output <png> and write a
//! same-resolution PNG. The engine and its neural model are installed separately.
use image::RgbaImage;
use std::{io::Read, path::PathBuf, process::{Command, Stdio}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, SystemTime, UNIX_EPOCH}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Sharpen, Deblur }

/// Restoration changes RGB detail, never the source photo's transparency.
pub fn preserve_alpha(original: &RgbaImage, restored: &mut RgbaImage) {
    for (source, target) in original.pixels().zip(restored.pixels_mut()) {
        target[3] = source[3];
    }
}

impl Operation {
    pub fn name(self) -> &'static str { match self { Self::Sharpen => "Sharpen", Self::Deblur => "Deblur" } }
    fn executable(self) -> &'static str { match self { Self::Sharpen => "sharpen.exe", Self::Deblur => "deblur.exe" } }
}

pub fn engine_path(operation: Operation) -> PathBuf {
    let relative = PathBuf::from("tools").join("restoration").join(operation.executable());
    if let Some(path) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(&relative))) {
        if path.is_file() { return path; }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Luminance-only detail transfer. The adjusted original supplies the colors and
/// base structure; Real-ESRGAN contributes only a limited high-frequency signal.
/// Keeping original detail prevents a smooth neural output from softening edges.
fn preserve_color_sharpen(original: &RgbaImage, neural: &RgbaImage) -> RgbaImage {
    let original_blur = image::imageops::blur(original, 1.15);
    let neural_blur = image::imageops::blur(neural, 1.15);
    let mut output = original.clone();
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let base = original.get_pixel(x, y);
        let ai = neural.get_pixel(x, y);
        let ob = original_blur.get_pixel(x, y);
        let nb = neural_blur.get_pixel(x, y);
        let luma = |p: &image::Rgba<u8>| -> f32 {
            0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
        };
        let original_detail = luma(base) - luma(ob);
        let neural_detail = luma(ai) - luma(nb);
        // Reject tiny differences (mostly flat-area noise); protect against halos.
        let detail = if original_detail.abs() < 1.5 && neural_detail.abs() < 1.5 {
            0.0
        } else {
            (0.85 * original_detail + 0.30 * neural_detail).clamp(-24.0, 24.0)
        };
        for channel in 0..3 {
            pixel[channel] = (base[channel] as f32 + detail).round().clamp(0.0, 255.0) as u8;
        }
        pixel[3] = base[3];
    }
    output
}

/// Neural-assisted luminance sharpening using Real-ESRGAN detail only.
/// Original RGB colors, alpha, and base structure are preserved.
pub fn sharpen(
    original: RgbaImage,
    settings: crate::adjustments::Adjustments,
    gpu_id: i32,
    tile_size: u32,
    cancel: Arc<AtomicBool>,
) -> Result<crate::ai_upscale::UpscaleResult, String> {
    let width = original.width();
    let height = original.height();
    let mut baseline = original.clone();
    for pixel in baseline.pixels_mut() { settings.apply_pixel(pixel); }
    let mut result = crate::ai_upscale::upscale(
        original, settings, 2,
        crate::ai_upscale::UpscaleOptions {
            model: "realesrgan-x4plus".into(), gpu_id, tile_size
        }, cancel.clone()
    )?;
    if cancel.load(Ordering::Relaxed) { return Err("AI sharpen cancelled".into()); }
    let enhanced = image::open(&result.ai_path)
        .map_err(|e| format!("Cannot decode enhanced image: {e}"))?.to_rgba8();
    let resized_neural = image::imageops::resize(
        &enhanced, width, height, image::imageops::FilterType::Lanczos3
    );
    let restored = preserve_color_sharpen(&baseline, &resized_neural);
    if cancel.load(Ordering::Relaxed) { return Err("AI sharpen cancelled".into()); }
    let ai_path = result.work_dir.join("sharpened.png");
    let baseline_path = result.work_dir.join("sharpen_original.png");
    restored.save(&ai_path).map_err(|e| format!("Cannot retain sharpened result: {e}"))?;
    baseline.save(&baseline_path).map_err(|e| format!("Cannot retain original comparison: {e}"))?;
    let longest = width.max(height);
    let preview_size = if longest > 2048 {
        let ratio = 2048.0 / longest as f64;
        [((width as f64 * ratio).round() as u32).max(1),
         ((height as f64 * ratio).round() as u32).max(1)]
    } else { [width, height] };
    result.preview = image::imageops::resize(&restored, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
    result.baseline_preview = image::imageops::resize(&baseline, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
    result.dimensions = [width, height];
    result.ai_path = ai_path;
    result.baseline_path = baseline_path;
    Ok(result)
}

pub fn run(
    mut original: RgbaImage,
    settings: crate::adjustments::Adjustments,
    operation: Operation,
    cancel: Arc<AtomicBool>,
) -> Result<crate::ai_upscale::UpscaleResult, String> {
    if cancel.load(Ordering::Relaxed) { return Err(format!("AI {} cancelled", operation.name())); }
    if operation != Operation::Deblur { return Err("Use the dedicated Real-ESRGAN sharpen pipeline".into()); }
    let relative = PathBuf::from("tools").join("restoration");
    let bundled = std::env::current_exe().ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&relative)));
    let folder = bundled.filter(|dir| dir.join("nafnet_deblur.py").is_file())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative));
    let mut script = folder.join("nafnet_deblur.py");
    if !script.is_file() {
        let runner=crate::dependencies::root().join("runner");
        std::fs::create_dir_all(&runner).map_err(|e|format!("Cannot prepare deblur runner: {e}"))?;
        script=runner.join("nafnet_deblur.py");
        std::fs::write(&script,include_str!("../tools/restoration/nafnet_deblur.py")).map_err(|e|format!("Cannot prepare deblur runner: {e}"))?;
    }
    let model = crate::dependencies::deblur_model_path().unwrap_or_else(||folder.join("deblurring_nafnet_2025may.onnx"));
    if !script.is_file() { return Err(format!("NAFNet runner missing: {}", script.display())); }
    if !model.is_file() { return Err(format!("Download the OpenCV NAFNet ONNX model to {}", model.display())); }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let work = std::env::temp_dir().join(format!("pixelforge_restore_{}_{}", std::process::id(), stamp));
    std::fs::create_dir_all(&work).map_err(|e| format!("Cannot create restoration workspace: {e}"))?;
    let result = (|| {
        for pixel in original.pixels_mut() { settings.apply_pixel(pixel); }
        if cancel.load(Ordering::Relaxed) { return Err("AI restoration cancelled".into()); }
        let input = work.join("input.png");
        let output = work.join("output.png");
        original.save(&input).map_err(|e| format!("Cannot prepare AI restoration input: {e}"))?;
        let mut command=if let Some(python)=crate::dependencies::deblur_python() {Command::new(python)} else {let mut command=Command::new("py");command.arg("-3");command};
        #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
        let mut process = command.arg(&script)
            .arg("--input").arg(&input).arg("--output").arg(&output)
            .arg("--model").arg(&model)
            .stdout(Stdio::null()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("Cannot launch Python NAFNet runner: {e}. Install Python 3 with the Windows py launcher."))?;
        let stderr = process.stderr.take().ok_or("Cannot capture restoration diagnostics")?;
        let reader = std::thread::spawn(move || {
            let mut pipe = stderr;
            let mut tail = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        tail.extend_from_slice(&buf[..n]);
                        if tail.len() > 16384 { tail.drain(..tail.len()-16384); }
                    }
                }
            }
            String::from_utf8_lossy(&tail).into_owned()
        });
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = process.kill(); let _ = process.wait(); let _ = reader.join();
                return Err(format!("AI {} cancelled", operation.name()));
            }
            match process.try_wait() {
                Ok(Some(status)) => {
                    let log = reader.join().unwrap_or_default();
                    if !status.success() { return Err(format!("AI {} failed ({status}): {}", operation.name(), log.trim())); }
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => {
                    let _ = process.kill(); let _ = process.wait(); let _ = reader.join();
                    return Err(format!("AI restoration process error: {e}"));
                }
            }
        }
        if cancel.load(Ordering::Relaxed) { return Err("AI restoration cancelled".into()); }
        let mut restored = image::open(&output).map_err(|e| format!("Cannot decode neural restoration output: {e}"))?.to_rgba8();
        if restored.dimensions() != original.dimensions() {
            return Err(format!("AI {} must preserve image dimensions", operation.name()));
        }
        preserve_alpha(&original, &mut restored);
        let baseline_path = work.join("original.png");
        let ai_path = work.join("restored.png");
        original.save(&baseline_path).map_err(|e| e.to_string())?;
        restored.save(&ai_path).map_err(|e| e.to_string())?;
        let dimensions = [restored.width(), restored.height()];
        let longest = dimensions[0].max(dimensions[1]);
        let preview_size = if longest > 2048 {
            let ratio = 2048.0 / longest as f64;
            [((dimensions[0] as f64 * ratio).round() as u32).max(1),
             ((dimensions[1] as f64 * ratio).round() as u32).max(1)]
        } else { dimensions };
        let preview = image::imageops::resize(&restored, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
        let baseline_preview = image::imageops::resize(&original, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
        Ok(crate::ai_upscale::UpscaleResult { work_dir: work.clone(), ai_path, baseline_path, preview, baseline_preview, dimensions })
    })();
    if result.is_err() { let _ = std::fs::remove_dir_all(&work); }
    result
}

#[cfg(test)]
mod sharpening_tests {
    use super::*;

    #[test]
    fn opaque_engine_output_keeps_source_transparency_and_restored_rgb() {
        let source = RgbaImage::from_fn(3, 1, |x, _| image::Rgba([10, 20, 30, [0, 127, 255][x as usize]]));
        let mut output = RgbaImage::from_pixel(3, 1, image::Rgba([40, 50, 60, 255]));
        preserve_alpha(&source, &mut output);
        for (x, pixel) in output.pixels().enumerate() {
            assert_eq!(*pixel, image::Rgba([40, 50, 60, [0, 127, 255][x]]));
        }
    }

    #[test]
    fn flat_image_keeps_original_color_and_alpha() {
        let original = RgbaImage::from_pixel(48, 48, image::Rgba([145, 102, 58, 173]));
        let neural = RgbaImage::from_pixel(48, 48, image::Rgba([210, 210, 210, 255]));
        assert_eq!(preserve_color_sharpen(&original, &neural), original);
    }

    #[test]
    fn edge_sharpening_preserves_alpha_and_changes_luminance() {
        let original = RgbaImage::from_fn(64, 64, |x, _| {
            if x < 32 { image::Rgba([70, 110, 150, 99]) }
            else { image::Rgba([140, 180, 220, 99]) }
        });
        let output = preserve_color_sharpen(&original, &original);
        assert!(output.pixels().all(|p| p[3] == 99));
        assert_ne!(output, original);
        // Away from edges the input color is preserved.
        assert_eq!(output.get_pixel(10, 10), original.get_pixel(10, 10));
    }
}
