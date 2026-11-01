//! Download curated SDXL checkpoints into a user-selected ComfyUI checkpoint folder.
use std::{fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::Duration};

pub struct Model {
    pub name: &'static str,
    pub keyword: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub description: &'static str,
    pub size: u64,
}
pub const MODELS: &[Model] = &[
    Model { name: "RealVisXL V5 fp16", keyword: "realvis", file: "RealVisXL_V5.0_fp16.safetensors",
        url: "https://huggingface.co/SG161222/RealVisXL_V5.0/resolve/main/RealVisXL_V5.0_fp16.safetensors",
        description: "Natural photography, portraits and landscapes", size: 6_940_000_000 },
    Model { name: "Juggernaut XL v9", keyword: "juggernaut", file: "Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",
        url: "https://huggingface.co/RunDiffusion/Juggernaut-XL-v9/resolve/main/Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",
        description: "Detailed photography and cinematic imagery", size: 7_110_000_000 },
];
pub fn install(index: usize, folder: PathBuf, progress: Arc<AtomicU64>,
    total: Arc<AtomicU64>, cancel: Arc<AtomicBool>) -> Result<PathBuf, String> {
    let model = MODELS.get(index).ok_or("Unknown model")?;
    if !folder.is_dir() { return Err("Checkpoint folder does not exist".into()); }
    let destination = folder.join(model.file);
    if destination.exists() { return Err("Model file already exists; refusing to overwrite".into()); }
    let partial = folder.join(format!("{}.part", model.file));
    if partial.exists() { return Err("An unfinished download exists. Move or delete the .part file before retrying.".into()); }
    let free = fs2::available_space(&folder).map_err(|e| format!("Cannot check free disk space: {e}"))?;
    if free < model.size + 1_000_000_000 { return Err("Not enough free space (model size plus 1 GB safety margin)".into()); }
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(120))
        .build().map_err(|e| e.to_string())?;
    let mut response = client.get(model.url).send().map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?;
    let length = response.content_length().ok_or("Server did not provide a file size")?;
    if length < 5_000_000_000 || length > 12_000_000_000 {
        return Err(format!("Unexpected checkpoint size: {length} bytes"));
    }
    if free < length + 1_000_000_000 { return Err("Insufficient disk space for download".into()); }
    total.store(length, Ordering::Relaxed);
    let mut file = File::create(&partial).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut buffer = [0u8; 256 * 1024];
        let mut written = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) { return Err("Download cancelled".into()); }
            let count = response.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 { break; }
            file.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
            written += count as u64;
            progress.store(written, Ordering::Relaxed);
        }
        file.sync_all().map_err(|e| e.to_string())?;
        if written != length { return Err(format!("Incomplete download: {written} of {length} bytes")); }
        Ok(())
    })();
    drop(file);
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    fs::rename(&partial, &destination).map_err(|e| format!("Cannot finalize checkpoint: {e}"))?;
    Ok(destination)
}
