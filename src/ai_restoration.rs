//! Adapter for dedicated neural sharpening and deblurring executables.
//! Each external engine must accept --input <png> --output <png> and write a
//! same-resolution PNG. The engine and its neural model are installed separately.
use image::RgbaImage;
use std::{io::Read, path::PathBuf, process::{Command, Stdio}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, SystemTime, UNIX_EPOCH}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Sharpen, Deblur }

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

/// Neural detail enhancement: Real-ESRGAN inference followed by same-size reconstruction.
/// This is not a separately trained sharpening model.
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
    let restored = image::imageops::resize(
        &enhanced, width, height, image::imageops::FilterType::Lanczos3
    );
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
    let engine = engine_path(operation);
    if !engine.is_file() {
        return Err(format!("AI {} neural inference engine not installed at {}. This feature requires a compatible external model runner; no conventional filter was substituted.", operation.name(), engine.display()));
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let work = std::env::temp_dir().join(format!("pixelforge_restore_{}_{}", std::process::id(), stamp));
    std::fs::create_dir_all(&work).map_err(|e| format!("Cannot create restoration workspace: {e}"))?;
    let result = (|| {
        for pixel in original.pixels_mut() { settings.apply_pixel(pixel); }
        if cancel.load(Ordering::Relaxed) { return Err("AI restoration cancelled".into()); }
        let input = work.join("input.png");
        let output = work.join("output.png");
        original.save(&input).map_err(|e| format!("Cannot prepare AI restoration input: {e}"))?;
        let mut process = Command::new(&engine)
            .arg("--input").arg(&input).arg("--output").arg(&output)
            .stdout(Stdio::null()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("Cannot launch AI {}: {e}", operation.name()))?;
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
        let restored = image::open(&output).map_err(|e| format!("Cannot decode neural restoration output: {e}"))?.to_rgba8();
        if restored.dimensions() != original.dimensions() {
            return Err(format!("AI {} must preserve image dimensions", operation.name()));
        }
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
