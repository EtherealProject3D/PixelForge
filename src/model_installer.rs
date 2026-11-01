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

/// Resolve ComfyUI Desktop's extra-model-paths YAML, then verify the folder
/// contains a checkpoint reported by the running backend.
pub fn detect_checkpoint_folder(known: &[String]) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(roaming) = std::env::var_os("APPDATA") {
        let configs = PathBuf::from(roaming).join("Comfy Desktop/instance-model-paths");
        if let Ok(files) = fs::read_dir(configs) {
            for file in files.flatten() {
                let path = file.path();
                if !matches!(path.extension().and_then(|x| x.to_str()), Some("yaml" | "yml")) { continue; }
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                        if let Some(sections) = config.as_mapping() {
                            for section in sections.values() {
                                let Some(mapping) = section.as_mapping() else { continue };
                                let base = mapping.get(serde_yaml::Value::String("base_path".into()))
                                    .and_then(|v| v.as_str()).map(PathBuf::from);
                                let checkpoints = mapping.get(serde_yaml::Value::String("checkpoints".into()))
                                    .and_then(|v| v.as_str());
                                if let Some(checkpoints) = checkpoints {
                                    for line in checkpoints.lines().map(str::trim).filter(|x| !x.is_empty()) {
                                        let p = PathBuf::from(line);
                                        roots.push(if p.is_absolute() { p } else if let Some(base) = &base { base.join(p) } else { continue });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(local).join("Comfy-Desktop/ComfyUI-Installs");
        if let Ok(installs) = fs::read_dir(root) {
            for install in installs.flatten() {
                roots.push(install.path().join("ComfyUI/models/checkpoints"));
            }
        }
    }
    let mut matched = Vec::new();
    for root in roots {
        if !root.is_dir() { continue; }
        let found = fs::read_dir(&root).ok().is_some_and(|files| files.flatten().any(|entry| {
            let filename = entry.file_name().to_string_lossy().to_lowercase();
            known.iter().any(|model| {
                let name = Path::new(model).file_name()
                    .map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
                !name.is_empty() && filename == name
            })
        }));
        if found {
            let canonical = root.canonicalize().unwrap_or(root);
            if !matched.contains(&canonical) { matched.push(canonical); }
        }
    }
    if matched.len() == 1 { matched.pop() } else { None }
}
