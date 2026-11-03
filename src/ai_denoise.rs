use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use image::RgbaImage;

pub fn engine_path() -> PathBuf {
    if let Some(folder)=crate::dependencies::active_path(crate::dependencies::Component::Denoise) {return folder.join("waifu2x-ncnn-vulkan.exe");}
    let relative = PathBuf::from("tools").join("waifu2x").join("waifu2x-ncnn-vulkan.exe");
    if let Some(path) = std::env::current_exe().ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&relative))) {
        if path.is_file() { return path; }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

pub fn denoise(
    mut original: RgbaImage,
    settings: crate::adjustments::Adjustments,
    level: u32,
    gpu_id: i32,
    tile_size: u32,
    cancel: Arc<AtomicBool>,
) -> Result<crate::ai_upscale::UpscaleResult, String> {
    if level > 3 { return Err("Denoise level must be between 0 and 3".into()); }
    if tile_size != 0 && !(32..=1024).contains(&tile_size) {
        return Err("Invalid denoise tile size".into());
    }
    if cancel.load(Ordering::Relaxed) { return Err("AI denoise cancelled".into()); }
    let engine = engine_path();
    if !engine.is_file() {
        return Err(format!("Waifu2x denoise engine missing: {}. Install the official Windows release into tools/waifu2x.", engine.display()));
    }
    let engine_dir = engine.parent().ok_or("Invalid denoise engine location")?;
    // Cunet contains dedicated noise-only models for 1x processing.
    let models = engine_dir.join("models-cunet");
    for ext in ["param", "bin"] {
        let file = models.join(format!("noise{level}_model.{ext}"));
        if !file.is_file() { return Err(format!("AI denoise model missing: {}", file.display())); }
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let work = std::env::temp_dir().join(format!("pixelforge_denoise_{}_{}", std::process::id(), stamp));
    std::fs::create_dir_all(&work).map_err(|e| format!("Cannot create denoise work directory: {e}"))?;
    let result = (|| {
        for pixel in original.pixels_mut() { settings.apply_pixel(pixel); }
        if cancel.load(Ordering::Relaxed) { return Err("AI denoise cancelled".into()); }
        let input = work.join("input.png");
        let output = work.join("output.png");
        original.save(&input).map_err(|e| format!("Cannot prepare denoise input: {e}"))?;
        if cancel.load(Ordering::Relaxed) { return Err("AI denoise cancelled".into()); }
        let mut command = Command::new(&engine);
        command.current_dir(engine_dir)
            .arg("-i").arg(&input).arg("-o").arg(&output)
            .arg("-n").arg(level.to_string()).arg("-s").arg("1")
            .arg("-m").arg(&models).arg("-t").arg(tile_size.to_string());
        if gpu_id >= 0 { command.arg("-g").arg(gpu_id.to_string()); }
        let mut child = command.arg("-f").arg("png")
            .stdout(Stdio::null()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("Cannot launch AI denoise: {e}"))?;
        let stderr = child.stderr.take().ok_or("Cannot capture denoise diagnostics")?;
        let log_thread = std::thread::spawn(move || {
            let mut reader = stderr;
            let mut buffer = [0u8; 4096];
            let mut tail = Vec::new();
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        tail.extend_from_slice(&buffer[..n]);
                        if tail.len() > 16384 { tail.drain(..tail.len()-16384); }
                    }
                }
            }
            String::from_utf8_lossy(&tail).into_owned()
        });
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = log_thread.join();
                return Err("AI denoise cancelled".into());
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    let details = log_thread.join().unwrap_or_default();
                    if !status.success() {
                        return Err(format!("AI denoise failed ({status}): {}", details.trim()));
                    }
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = log_thread.join();
                    return Err(format!("Cannot monitor denoise engine: {e}"));
                }
            }
        }
        if cancel.load(Ordering::Relaxed) { return Err("AI denoise cancelled".into()); }
        let denoised = image::open(&output)
            .map_err(|e| format!("Cannot read AI denoise output: {e}"))?.to_rgba8();
        if denoised.dimensions() != original.dimensions() {
            return Err(format!("Denoise changed image dimensions: {:?} -> {:?}", original.dimensions(), denoised.dimensions()));
        }
        let baseline_path = work.join("original.png");
        let ai_path = work.join("denoised.png");
        original.save(&baseline_path).map_err(|e| format!("Cannot save original comparison: {e}"))?;
        denoised.save(&ai_path).map_err(|e| format!("Cannot save denoised image: {e}"))?;
        let dimensions = [denoised.width(), denoised.height()];
        let longest = dimensions[0].max(dimensions[1]);
        let preview_size = if longest > 2048 {
            let ratio = 2048.0 / longest as f64;
            [((dimensions[0] as f64 * ratio).round() as u32).max(1),
             ((dimensions[1] as f64 * ratio).round() as u32).max(1)]
        } else { dimensions };
        let preview = image::imageops::resize(&denoised, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
        let baseline_preview = image::imageops::resize(&original, preview_size[0], preview_size[1], image::imageops::FilterType::Triangle);
        if cancel.load(Ordering::Relaxed) { return Err("AI denoise cancelled".into()); }
        Ok(crate::ai_upscale::UpscaleResult {
            work_dir: work.clone(), ai_path, baseline_path, preview, baseline_preview, dimensions
        })
    })();
    if result.is_err() { let _ = std::fs::remove_dir_all(&work); }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_location_is_stable() {
        assert!(super::engine_path().to_string_lossy().contains("waifu2x-ncnn-vulkan.exe"));
    }
}
