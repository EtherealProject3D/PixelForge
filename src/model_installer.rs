//! Shared installer for curated ComfyUI checkpoints and model bundles.
use std::{fs::{self, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::Duration};

pub struct ModelFile {
    pub directory: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub min: u64,
    pub max: u64,
    pub sha256: Option<&'static str>,
}
pub struct Model {
    pub name: &'static str,
    pub keyword: &'static str,
    pub description: &'static str,
    pub page: &'static str,
    pub editing: bool,
    pub generation: bool,
    pub files: &'static [ModelFile],
}
impl Model {
    pub fn bundle(&self) -> bool { self.files.iter().any(|file|file.directory!="checkpoints") }
    pub fn show_compatibility(&self, ui: &mut egui::Ui) {
        let disk_gb = self.files.iter().map(|file| file.size).sum::<u64>() as f64 / 1e9;
        ui.label(format!("Download size: about {disk_gb:.1} GB before reusing shared files."));
        if self.keyword == "flux1-dev" {
            ui.label("Requires a ComfyUI backend with FLUX.1 and FP8 support. This checkpoint is distributed under the FLUX.1 Dev non-commercial license.");
        } else if self.bundle() {
            let requirements = match self.keyword {
                "fluxklein4b" => "Requires ComfyUI with the FLUX.2 text encoder and FP8 model loading.",
                "qwen" | "qwen2511" => "Requires ComfyUI with Qwen image edit nodes, the Qwen text encoder, and FP8 model loading.",
                "cyberrealistic_zit" => "Requires ComfyUI with the Z-Image workflow nodes and the Qwen 3 text encoder.",
                _ => "Requires ComfyUI with the model’s loaders and precision support.",
            };
            ui.label(requirements);
            ui.label("NVIDIA: depends on GPU and PyTorch precision support. AMD, Intel and CPU: not yet verified in PixelForge.");
        } else {
            ui.label("Requires ComfyUI with SDXL support. NVIDIA: expected with a compatible PyTorch runtime. AMD and Intel: not yet verified in PixelForge.");
            ui.label("CPU: ComfyUI offers CPU mode. This checkpoint has not been tested on CPU in PixelForge.");
        }
        ui.label("RAM and VRAM needs depend on resolution, precision and offloading. A minimum has not been verified.");
    }
}
pub const MODELS: &[Model] = &[
    Model {name:crate::ai_generation::Z_IMAGE,keyword:"cyberrealistic_zit",description:"Fast photographic generation with natural lighting and detail. About 20.7 GB total, or 12.6 GB if the FLUX Qwen encoder is installed. Start at 8–9 steps, CFG 1; negative prompts are unused.",page:"https://huggingface.co/cyberdelia/cyberrealistic_zimage",editing:false,generation:true,
        files:&[
            ModelFile {directory:"diffusion_models",file:"CyberRealistic_zit_v4.0.safetensors",url:"https://huggingface.co/cyberdelia/cyberrealistic_zimage/resolve/2b084ad2e15e6406e50b08fc16ca3229ddc8981b/CyberRealistic_zit_v4.0.safetensors",size:12_309_876_512,min:12_309_876_512,max:12_309_876_512,sha256:None},
            ModelFile {directory:"text_encoders",file:"qwen_3_4b.safetensors",url:"https://huggingface.co/Comfy-Org/z_image_turbo/resolve/main/split_files/text_encoders/qwen_3_4b.safetensors",size:8_044_982_048,min:7_500_000_000,max:9_000_000_000,sha256:None},
            ModelFile {directory:"vae",file:"ae.safetensors",url:"https://huggingface.co/Comfy-Org/z_image_turbo/resolve/main/split_files/vae/ae.safetensors",size:335_304_388,min:335_304_388,max:335_304_388,sha256:None},
        ]},
    Model {name:"CyberRealistic XL v10 FP16",keyword:"cyberrealisticxl",description:"Photographic people, natural skin detail and cinematic lighting; 30+ steps, CFG 3–5. About 6.9 GB, with built-in VAE.",page:"https://cyberdelia.nl/models/cyberrealistic-xl",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"CyberRealisticXLPlay_V10.0_FP16.safetensors",url:"https://huggingface.co/cyberdelia/CyberRealisticXL/resolve/c04ae13b465d19763561a834d625a3d504a4f315/CyberRealisticXLPlay_V10.0_FP16.safetensors",size:6_938_041_288,min:6_938_041_288,max:6_938_041_288,sha256:None}]},
    Model {name:"CyberRealistic Pony v18 CoreShift fp16",keyword:"cyberrealisticpony",description:"Pony XL character realism; 30+ steps, CFG 5",page:"https://huggingface.co/cyberdelia/CyberRealisticPony",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"CyberRealisticPony_V18.0_F16.safetensors",url:"https://huggingface.co/cyberdelia/CyberRealisticPony/resolve/main/CyberRealisticPony_V18.0_F16.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000,sha256:None}]},
    Model {name:"Pony Diffusion V6 XL",keyword:"ponydiffusionv6xl",description:"Pony XL illustration checkpoint. Start at 30 steps and CFG 5.",page:"https://huggingface.co/AstraliteHeart/pony-diffusion-v6",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"ponyDiffusionV6XL_v6StartWithThisOne.safetensors",url:"https://huggingface.co/AstraliteHeart/pony-diffusion-v6/resolve/5ec9c05863255568f1b59753e3838107befaa712/v6.safetensors",size:6_938_041_050,min:6_938_041_050,max:6_938_041_050,sha256:Some("67ab2fd8ec439a89b3fedb15cc65f54336af163c7eb5e4f2acc98f090a29b0b3")}]},
    Model {name:"FLUX.1 Dev (FP8)",keyword:"flux1-dev",description:"Single-file FLUX.1 checkpoint with optional local LoRA support.",page:"https://huggingface.co/Comfy-Org/Flux-1-Dev",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"flux1-dev-fp8.safetensors",url:"https://huggingface.co/Comfy-Org/Flux-1-Dev/resolve/57bb73f19aa46d9111f2b7eeab62c3c3d09f2ced/flux1-dev-fp8.safetensors",size:17_246_524_772,min:17_246_524_772,max:17_246_524_772,sha256:Some("8e91b68084b53a7fc44ed2a3756d821e355ac1a7b6fe29be760c1db532f3d88a")}]},
    Model {name:"RealVisXL V5 fp16",keyword:"realvis",description:"Natural photography, portraits and landscapes",page:"https://huggingface.co/SG161222/RealVisXL_V5.0",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"RealVisXL_V5.0_fp16.safetensors",url:"https://huggingface.co/SG161222/RealVisXL_V5.0/resolve/main/RealVisXL_V5.0_fp16.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000,sha256:None}]},
    Model {name:"Juggernaut XL v9",keyword:"juggernaut",description:"Detailed photography and cinematic imagery",page:"https://huggingface.co/RunDiffusion/Juggernaut-XL-v9",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",url:"https://huggingface.co/RunDiffusion/Juggernaut-XL-v9/resolve/main/Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",size:7110000000,min:5_000_000_000,max:12_000_000_000,sha256:None}]},
    Model {name:"SDXL Base 1.0",keyword:"sd_xl_base",description:"General-purpose generation and illustration",page:"https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"sd_xl_base_1.0.safetensors",url:"https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/main/sd_xl_base_1.0.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000,sha256:None}]},
    Model {name:"Qwen-Image-Edit 2509 (FP8)",keyword:"qwen",description:"Instruction-based image editing. Installs all three required files (about 30 GB).",page:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit",editing:true,generation:false,
        files:&[
            ModelFile {directory:"diffusion_models",file:"qwen_image_edit_2509_fp8_e4m3fn.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit/resolve/main/split_files/diffusion_models/qwen_image_edit_2509_fp8_e4m3fn.safetensors",size:20_500_000_000,min:18_000_000_000,max:24_000_000_000,sha256:None},
            ModelFile {directory:"text_encoders",file:"qwen_2.5_vl_7b_fp8_scaled.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors",size:9_400_000_000,min:8_000_000_000,max:12_000_000_000,sha256:None},
            ModelFile {directory:"vae",file:"qwen_image_vae.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/vae/qwen_image_vae.safetensors",size:250_000_000,min:200_000_000,max:500_000_000,sha256:None},
        ]},
    Model {name:"Qwen-Image-Edit 2511 (FP8)",keyword:"qwen2511",description:"Newer instruction editor with improved consistency. About 20.5 GB extra when the existing Qwen encoder/VAE are present.",page:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit",editing:true,generation:false,
        files:&[
            ModelFile {directory:"diffusion_models",file:"qwen_image_edit_2511_fp8mixed.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit/resolve/main/split_files/diffusion_models/qwen_image_edit_2511_fp8mixed.safetensors",size:20_600_000_000,min:18_000_000_000,max:24_000_000_000,sha256:None},
            ModelFile {directory:"text_encoders",file:"qwen_2.5_vl_7b_fp8_scaled.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors",size:9_400_000_000,min:8_000_000_000,max:12_000_000_000,sha256:None},
            ModelFile {directory:"vae",file:"qwen_image_vae.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/vae/qwen_image_vae.safetensors",size:250_000_000,min:200_000_000,max:500_000_000,sha256:None},
        ]},
    Model {name:"FLUX.2 Klein 4B (FP8)",keyword:"fluxklein4b",description:"Local generation and instruction editing. Distilled 4-step model; about 12.5 GB total. Uses memory offloading when needed.",page:"https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8",editing:true,generation:true,
        files:&[
            ModelFile {directory:"diffusion_models",file:"flux-2-klein-4b-fp8.safetensors",url:"https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8/resolve/main/flux-2-klein-4b-fp8.safetensors",size:4_100_000_000,min:3_500_000_000,max:5_000_000_000,sha256:None},
            ModelFile {directory:"text_encoders",file:"qwen_3_4b.safetensors",url:"https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/text_encoders/qwen_3_4b.safetensors",size:8_100_000_000,min:7_500_000_000,max:9_000_000_000,sha256:None},
            ModelFile {directory:"vae",file:"flux2-vae.safetensors",url:"https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/vae/flux2-vae.safetensors",size:340_000_000,min:300_000_000,max:450_000_000,sha256:None},
        ]},
];

fn valid_file(file:&ModelFile,path:&Path) -> bool {
    fs::metadata(path).is_ok_and(|m|m.is_file() && (file.min..=file.max).contains(&m.len()))
}
pub fn installed_files(model:&Model,root:&Path) -> usize {
    model.files.iter().filter(|file|valid_file(file,&root.join(file.directory).join(file.file))).count()
}

fn partial_download_size(file: &ModelFile, destination: &Path) -> Result<u64, String> {
    let partial = destination.with_extension("safetensors.part");
    match fs::symlink_metadata(&partial) {
        Ok(metadata)
            if file.sha256.is_some() && metadata.is_file() && metadata.len() <= file.max =>
        {
            Ok(metadata.len())
        }
        Ok(_) => Err(format!(
            "Cannot safely resume {}. Move the partial file aside before retrying.",
            partial.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.to_string()),
    }
}

pub fn resumable_bytes(model: &Model, root: &Path) -> u64 {
    model
        .files
        .iter()
        .map(|file| {
            let destination = root.join(file.directory).join(file.file);
            if destination.exists() {
                return if valid_file(file, &destination) {
                    Ok(0)
                } else {
                    Err("Invalid existing model".into())
                };
            }
            partial_download_size(file, &destination)
        })
        .sum::<Result<u64, String>>()
        .unwrap_or(0)
}

pub fn install(index:usize,root:PathBuf,progress:Arc<AtomicU64>,total:Arc<AtomicU64>,cancel:Arc<AtomicBool>) -> Result<PathBuf,String> {
    install_model(MODELS.get(index).ok_or("Unknown model")?,root,progress,total,cancel)
}

fn install_model(
    model: &Model,
    root: PathBuf,
    progress: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    if !root.is_dir() {
        return Err("ComfyUI models folder does not exist".into());
    }
    let mut missing = Vec::new();
    let mut remaining = 0u64;
    for file in model.files {
        let path = root.join(file.directory).join(file.file);
        if path.exists() {
            if !valid_file(file, &path) {
                return Err(format!(
                    "Existing model file has an unexpected size; inspect it before retrying: {}",
                    path.display()
                ));
            }
        } else {
            let offset = partial_download_size(file, &path)?;
            remaining += file.size.saturating_sub(offset);
            missing.push(file);
        }
    }
    progress.store(0, Ordering::Relaxed);
    total.store(
        missing.iter().map(|file| file.size).sum(),
        Ordering::Relaxed,
    );
    if missing.is_empty() {
        return Ok(root);
    }
    let free =
        fs2::available_space(&root).map_err(|e| format!("Cannot check free disk space: {e}"))?;
    if remaining > 0 && free < remaining + 1_000_000_000 {
        return Err(
            "Not enough free space for the remaining download plus a 1 GB safety margin".into(),
        );
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(24 * 3600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut completed = 0;
    for file in missing {
        if cancel.load(Ordering::Relaxed) {
            return Err("Download cancelled".into());
        }
        let folder = root.join(file.directory);
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let destination = folder.join(file.file);
        let partial = destination.with_extension("safetensors.part");
        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(file.sha256.is_some())
            .truncate(false)
            .create_new(file.sha256.is_none())
            .open(&partial)
            .map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&output)
            .map_err(|e| format!("Another download is using {}: {e}", partial.display()))?;

        let result = (|| -> Result<(u64, bool), String> {
            let mut written = output.metadata().map_err(|e| e.to_string())?.len();
            if written > file.max {
                return Err("Partial download is larger than the expected model".into());
            }
            progress.store(completed + written, Ordering::Relaxed);
            if file.sha256.is_none() || written != file.size {
                let mut request = client.get(file.url);
                if written > 0 {
                    request = request.header(reqwest::header::RANGE, format!("bytes={written}-"));
                }
                let mut response = request
                    .send()
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?;
                let length = download_length(&response, written)?;
                if !(file.min..=file.max).contains(&length) {
                    return Err(format!("Unexpected size for {}: {length} bytes", file.file));
                }
                let available = fs2::available_space(&folder).map_err(|e| e.to_string())?;
                if available < length - written + 1_000_000_000 {
                    return Err("Insufficient disk space for the remaining download".into());
                }
                let previous = total.load(Ordering::Relaxed);
                total.store(previous - file.size + length, Ordering::Relaxed);
                output.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
                let mut buffer = [0u8; 256 * 1024];
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Download cancelled".into());
                    }
                    let count = response.read(&mut buffer).map_err(|e| e.to_string())?;
                    if count == 0 {
                        break;
                    }
                    if written + count as u64 > length {
                        return Err("Download exceeded expected file size".into());
                    }
                    output
                        .write_all(&buffer[..count])
                        .map_err(|e| e.to_string())?;
                    written += count as u64;
                    progress.store(completed + written, Ordering::Relaxed);
                }
                if written != length {
                    return Err(format!("Incomplete download: {written} of {length} bytes"));
                }
            }
            output.sync_all().map_err(|e| e.to_string())?;
            if cancel.load(Ordering::Relaxed) {
                return Err("Download cancelled".into());
            }
            let valid = match file.sha256 {
                Some(expected) => matches_model_hash(&mut output, expected, &cancel)?,
                None => true,
            };
            Ok((written, valid))
        })();
        drop(output);
        let (length, valid) = match result {
            Ok(result) => result,
            Err(error) => {
                if file.sha256.is_none() {
                    let _ = fs::remove_file(&partial);
                }
                return Err(error);
            }
        };
        if !valid {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let invalid = partial.with_extension(format!("part.invalid-{timestamp}"));
            publish_download(&partial, &invalid).map_err(|e| {
                format!(
                    "Checksum verification failed for {}. {e}",
                    partial.display()
                )
            })?;
            return Err(format!(
                "Checksum verification failed. Saved the invalid download as {}. Retry to start fresh.",
                invalid.display()
            ));
        }
        publish_download(&partial, &destination)?;
        completed += length;
    }
    Ok(root)
}

fn download_length(response: &reqwest::blocking::Response, offset: u64) -> Result<u64, String> {
    let length = response
        .content_length()
        .ok_or("Server did not provide a file size")?;
    if offset == 0 && response.status() == reqwest::StatusCode::OK {
        return Ok(length);
    }
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(
            "The server did not honor the resume request. The partial download was preserved."
                .into(),
        );
    }
    let total = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .and_then(|value| value.split_once('/'))
        .and_then(|(range, total)| {
            let (first, last) = range.split_once('-')?;
            Some((
                first.parse::<u64>().ok()?,
                last.parse::<u64>().ok()?,
                total.parse::<u64>().ok()?,
            ))
        })
        .filter(|&(first, last, total)| {
            first == offset
                && last.checked_add(1) == Some(total)
                && total.checked_sub(offset) == Some(length)
        })
        .map(|(_, _, total)| total)
        .ok_or(
            "The server returned an unexpected byte range. The partial download was preserved.",
        )?;
    Ok(total)
}

fn matches_model_hash(
    file: &mut fs::File,
    expected: &str,
    cancel: &AtomicBool,
) -> Result<bool, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0u8; 256 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Download cancelled".into());
        }
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let actual: String = hash
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(actual == expected)
}

fn publish_download(partial:&Path,destination:&Path) -> Result<(),String> {
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        #[link(name="kernel32")]
        unsafe extern "system" {fn MoveFileW(existing:*const u16,new:*const u16)->i32;}
        let from:Vec<u16>=partial.as_os_str().encode_wide().chain(Some(0)).collect();
        let to:Vec<u16>=destination.as_os_str().encode_wide().chain(Some(0)).collect();
        // MoveFileW does not overwrite an existing destination, even after a race.
        if unsafe {MoveFileW(from.as_ptr(),to.as_ptr())}==0 {
            return Err(format!("Cannot finalize model: {}",std::io::Error::last_os_error()));
        }
    }
    #[cfg(not(windows))] {
        fs::hard_link(partial,destination).map_err(|e|format!("Cannot finalize model: {e}"))?;
        fs::remove_file(partial).map_err(|e|e.to_string())?;
    }
    Ok(())
}

#[derive(Clone,Debug)]
pub struct RemovalFile {pub path:PathBuf,pub bytes:u64}
pub fn removal_plan(index:usize,root:&Path,backend_name:Option<&str>,include_shared:bool) -> Result<Vec<RemovalFile>,String> {
    let model=MODELS.get(index).ok_or("Unknown model")?;
    let root=root.canonicalize().map_err(|e|e.to_string())?;
    let mut candidates:Vec<PathBuf>=model.files.iter().filter(|f|!model.bundle() || include_shared || f.directory=="diffusion_models")
        .map(|f|root.join(f.directory).join(f.file)).collect();
    if !model.bundle() {
        if let Some(name)=backend_name {
            let relative=Path::new(name);
            if relative.components().any(|c|!matches!(c,std::path::Component::Normal(_))) || relative.extension().is_none_or(|ext|ext!="safetensors") {
                return Err("Invalid checkpoint path".into());
            }
            candidates.push(root.join("checkpoints").join(relative));
        }
    }
    let mut result=Vec::new();
    for path in candidates {
        if !path.exists() {continue;}
        let path=path.canonicalize().map_err(|e|e.to_string())?;
        if !path.starts_with(&root) || !path.is_file() {return Err("Model removal target is outside the selected models folder".into());}
        if !result.iter().any(|f:&RemovalFile|f.path==path) {
            result.push(RemovalFile {bytes:fs::metadata(&path).map_err(|e|e.to_string())?.len(),path});
        }
    }
    Ok(result)
}
pub fn remove_model_files(index:usize,root:&Path,backend_name:Option<&str>,include_shared:bool) -> Result<u64,String> {
    let plan=removal_plan(index,root,backend_name,include_shared)?;
    if plan.is_empty() {return Err("No matching files found in this models folder".into());}
    let mut removed=0;
    for file in plan {
        fs::remove_file(&file.path).map_err(|e|format!("Removed {removed} bytes, but could not remove {}: {e}",file.path.display()))?;
        removed+=file.bytes;
    }
    Ok(removed)
}

pub fn recommended_models_folder() -> PathBuf {
    crate::dependencies::root().join("models")
}

pub fn chosen_models_folder() -> Option<PathBuf> {
    let root = crate::profile::settings_root()?;
    read_models_folder(&root)
}

pub fn save_models_folder(folder: &Path) -> Result<(), String> {
    let root = crate::profile::settings_root().ok_or("APPDATA unavailable")?;
    write_models_folder(&root, folder)
}

fn read_models_folder(root: &Path) -> Option<PathBuf> {
    let bytes = fs::read(root.join("model-folder.json")).ok()?;
    serde_json::from_slice::<PathBuf>(&bytes)
        .ok()
        .filter(|path| path.is_absolute())
}

fn write_models_folder(root: &Path, folder: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let temp = root.join("model-folder.json.new");
    let bytes = serde_json::to_vec(folder).map_err(|error| error.to_string())?;
    fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    fs::rename(temp, root.join("model-folder.json")).map_err(|error| error.to_string())
}

pub fn detect_models_folder(known: &[String]) -> Option<PathBuf> {
    detect_checkpoint_folder(known).and_then(|p| p.parent().map(Path::to_path_buf))
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
                                let base = mapping.get(&serde_yaml::Value::String("base_path".into()))
                                    .and_then(|v| v.as_str()).map(PathBuf::from);
                                let checkpoints = mapping.get(&serde_yaml::Value::String("checkpoints".into()))
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
            (known.is_empty() && filename.ends_with(".safetensors")) || known.iter().any(|model| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    fn fixture() -> PathBuf {
        let path=std::env::temp_dir().join(format!("pixelforge-installer-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&path).unwrap();path
    }
    #[test]
    fn recommended_folder_matches_managed_backend_and_custom_choice_roundtrips() {
        assert_eq!(
            recommended_models_folder(),
            crate::dependencies::root().join("models")
        );
        let root = fixture();
        assert!(read_models_folder(&root).is_none());
        let first = root.join("First Models");
        let second = root.join("Second Models");
        write_models_folder(&root, &first).unwrap();
        assert_eq!(read_models_folder(&root), Some(first));
        write_models_folder(&root, &second).unwrap();
        assert_eq!(read_models_folder(&root), Some(second));
        fs::remove_file(root.join("model-folder.json")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn bundle_download_skips_existing_files_and_reports_aggregate_progress() {
        let root=fixture();fs::create_dir(root.join("vae")).unwrap();fs::write(root.join("vae/existing.safetensors"),b"keep").unwrap();
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let server=std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request=[0u8;4096];stream.read(&mut request).unwrap();
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata").unwrap();
            }
        });
        let url: &'static str=Box::leak(format!("http://{address}/model").into_boxed_str());
        let files=Box::leak(Box::new([
            ModelFile {directory:"vae",file:"existing.safetensors",url,size:4,min:4,max:4,sha256:None},
            ModelFile {directory:"diffusion_models",file:"new.safetensors",url,size:4,min:4,max:4,sha256:None},
            ModelFile {directory:"text_encoders",file:"new.safetensors",url,size:4,min:4,max:4,sha256:None},
        ]));
        let model=Model {name:"fixture",keyword:"fixture",description:"",page:"",editing:true,generation:false,files};
        let progress=Arc::new(AtomicU64::new(0));let total=Arc::new(AtomicU64::new(0));
        assert_eq!(install_model(&model,root.clone(),progress.clone(),total.clone(),Arc::new(AtomicBool::new(false))).unwrap(),root);
        server.join().unwrap();
        assert_eq!(installed_files(&model,&root),3);assert_eq!(progress.load(Ordering::Relaxed),8);assert_eq!(total.load(Ordering::Relaxed),8);
        assert_eq!(fs::read(root.join("vae/existing.safetensors")).unwrap(),b"keep");
        assert!(!root.join("diffusion_models/new.safetensors.part").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_bundle_keeps_completed_files_and_cleans_active_partial() {
        let root=fixture();let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let server=std::thread::spawn(move || {
            for n in 0..2 {
                let (mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request=[0u8;4096];stream.read(&mut request).unwrap();
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n").unwrap();
                stream.write_all(if n==0 {b"done"} else {b"xx"}).unwrap();
            }
        });
        let url: &'static str=Box::leak(format!("http://{address}/model").into_boxed_str());
        let files=Box::leak(Box::new([
            ModelFile {directory:"vae",file:"complete.safetensors",url,size:4,min:4,max:4,sha256:None},
            ModelFile {directory:"vae",file:"interrupted.safetensors",url,size:4,min:4,max:4,sha256:None},
        ]));
        let model=Model {name:"fixture",keyword:"fixture",description:"",page:"",editing:true,generation:false,files};
        assert!(install_model(&model,root.clone(),Arc::new(AtomicU64::new(0)),Arc::new(AtomicU64::new(0)),Arc::new(AtomicBool::new(false))).is_err());
        server.join().unwrap();
        assert_eq!(fs::read(root.join("vae/complete.safetensors")).unwrap(),b"done");
        assert!(!root.join("vae/interrupted.safetensors").exists());
        assert!(!root.join("vae/interrupted.safetensors.part").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checksum_pinned_download_resumes_without_replacing_the_partial_prefix() {
        for valid_range in [true, false] {
            let root = fixture();
            let folder = root.join("checkpoints");
            fs::create_dir(&folder).unwrap();
            let partial = folder.join("resumable.safetensors.part");
            fs::write(&partial, b"da").unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0u8; 4096];
                let count = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
                assert!(request.contains("range: bytes=2-"));
                let headers = if valid_range {
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 2-3/4"
                } else {
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-1/4"
                };
                write!(
                    stream,
                    "{headers}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nta"
                )
                .unwrap();
            });
            let url = Box::leak(format!("http://{address}/model").into_boxed_str());
            let model = verified_fixture(url);
            let progress = Arc::new(AtomicU64::new(0));
            let total = Arc::new(AtomicU64::new(0));
            let result = install_model(
                &model,
                root.clone(),
                progress.clone(),
                total.clone(),
                Arc::new(AtomicBool::new(false)),
            );
            server.join().unwrap();
            let destination = folder.join("resumable.safetensors");
            if valid_range {
                result.unwrap();
                assert_eq!(fs::read(&destination).unwrap(), b"data");
                assert!(!partial.exists());
                assert_eq!(progress.load(Ordering::Relaxed), 4);
                assert_eq!(total.load(Ordering::Relaxed), 4);
            } else {
                assert!(result.unwrap_err().contains("unexpected byte range"));
                assert_eq!(fs::read(&partial).unwrap(), b"da");
                assert!(!destination.exists());
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn verified_fixture(url: &'static str) -> Model {
        let files = Box::leak(Box::new([ModelFile {
            directory: "checkpoints",
            file: "resumable.safetensors",
            url,
            size: 4,
            min: 4,
            max: 4,
            sha256: Some("3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7"),
        }]));
        Model {
            name: "fixture",
            keyword: "fixture",
            description: "",
            page: "",
            editing: false,
            generation: true,
            files,
        }
    }

    #[test]
    fn library_reports_only_partials_that_can_be_resumed() {
        let root = fixture();
        let folder = root.join("checkpoints");
        fs::create_dir(&folder).unwrap();
        let model = verified_fixture("http://127.0.0.1:1/no-request");
        let destination = folder.join(model.files[0].file);
        let partial = destination.with_extension("safetensors.part");
        assert_eq!(resumable_bytes(&model, &root), 0);
        fs::write(&partial, b"da").unwrap();
        assert_eq!(resumable_bytes(&model, &root), 2);
        fs::write(&partial, b"too large").unwrap();
        assert_eq!(resumable_bytes(&model, &root), 0);
        fs::write(&partial, b"data").unwrap();
        assert_eq!(resumable_bytes(&model, &root), 4);
        fs::write(&destination, b"data").unwrap();
        assert_eq!(resumable_bytes(&model, &root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn full_partial_is_published_only_if_its_checksum_matches() {
        for contents in [b"data", b"nope"] {
            let root = fixture();
            let folder = root.join("checkpoints");
            fs::create_dir(&folder).unwrap();
            let partial = folder.join("resumable.safetensors.part");
            fs::write(&partial, contents).unwrap();
            let model = verified_fixture("http://127.0.0.1:1/no-request");
            let result = install_model(
                &model,
                root.clone(),
                Arc::new(AtomicU64::new(0)),
                Arc::new(AtomicU64::new(0)),
                Arc::new(AtomicBool::new(false)),
            );
            let destination = folder.join("resumable.safetensors");
            assert!(!partial.exists());
            if contents == b"data" {
                result.unwrap();
                assert_eq!(fs::read(&destination).unwrap(), b"data");
            } else {
                assert!(result.unwrap_err().contains("Checksum verification failed"));
                assert!(!destination.exists());
                let backup = fs::read_dir(&folder)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                assert_eq!(fs::read(backup).unwrap(), b"nope");
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn cancelled_install_never_downloads_and_invalid_existing_file_is_preserved() {
        let root=fixture();
        let files=Box::leak(Box::new([ModelFile {directory:"vae",file:"test.safetensors",url:"http://127.0.0.1:1/no-request",size:4,min:4,max:4,sha256:None}]));
        let model=Model {name:"fixture",keyword:"fixture",description:"",page:"",editing:true,generation:false,files};
        let run=|cancel|install_model(&model,root.clone(),Arc::new(AtomicU64::new(0)),Arc::new(AtomicU64::new(0)),Arc::new(AtomicBool::new(cancel)));
        assert!(run(true).unwrap_err().contains("cancelled"));
        fs::create_dir(root.join("vae")).unwrap();fs::write(root.join("vae/test.safetensors"),b"x").unwrap();
        assert!(run(false).unwrap_err().contains("Existing model file"));
        assert_eq!(fs::read(root.join("vae/test.safetensors")).unwrap(),b"x");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn publication_never_overwrites_an_existing_model() {
        let root=fixture();let partial=root.join("model.part");let destination=root.join("model");
        fs::write(&partial,b"new").unwrap();fs::write(&destination,b"old").unwrap();
        assert!(publish_download(&partial,&destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(),b"old");assert_eq!(fs::read(&partial).unwrap(),b"new");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removal_preserves_shared_qwen_files_by_default_and_rejects_traversal() {
        let root=fixture();let index=MODELS.iter().position(|m|m.editing).unwrap();let model=&MODELS[index];
        for file in model.files {fs::create_dir_all(root.join(file.directory)).unwrap();fs::write(root.join(file.directory).join(file.file),b"test").unwrap();}
        let plan=removal_plan(index,&root,None,false).unwrap();assert_eq!(plan.len(),1);
        assert!(plan[0].path.ends_with(model.files[0].file));
        assert_eq!(remove_model_files(index,&root,None,false).unwrap(),4);
        assert!(root.join(model.files[1].directory).join(model.files[1].file).is_file());
        assert!(root.join(model.files[2].directory).join(model.files[2].file).is_file());
        assert_eq!(removal_plan(index,&root,None,true).unwrap().len(),2);
        let checkpoint=MODELS.iter().position(|model|!model.bundle()).unwrap();
        assert!(removal_plan(checkpoint,&root,Some("../../outside.safetensors"),false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removal_uses_exact_detected_checkpoint_and_reports_disk_bytes() {
        let root=fixture();fs::create_dir(root.join("checkpoints")).unwrap();
        fs::write(root.join("checkpoints/alternate.safetensors"),b"abcdef").unwrap();
        let checkpoint=MODELS.iter().position(|model|!model.bundle()).unwrap();
        assert_eq!(remove_model_files(checkpoint,&root,Some("alternate.safetensors"),false).unwrap(),6);
        assert!(!root.join("checkpoints/alternate.safetensors").exists());fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn new_models_share_only_the_correct_companion_files() {
        let zimage=MODELS.iter().find(|m|m.keyword=="cyberrealistic_zit").unwrap();
        assert!(zimage.bundle() && zimage.generation && !zimage.editing);
        let qwen=MODELS.iter().find(|m|m.name==crate::ai_generation::QWEN_EDIT).unwrap();
        let newer=MODELS.iter().find(|m|m.name==crate::ai_generation::QWEN_EDIT_2511).unwrap();
        assert_ne!(qwen.files[0].file,newer.files[0].file);
        assert_eq!(qwen.files[1].file,newer.files[1].file);assert_eq!(qwen.files[2].file,newer.files[2].file);
        let flux=MODELS.iter().find(|m|m.name==crate::ai_generation::FLUX_KLEIN).unwrap();
        assert_eq!(zimage.files[1].file,flux.files[1].file);
        assert_ne!(zimage.files[2].file,flux.files[2].file);
        assert!(flux.generation&&flux.editing);
        assert_ne!(flux.files[1].file,qwen.files[1].file);assert_ne!(flux.files[2].file,qwen.files[2].file);
    }

    #[test]
    fn library_contains_generation_models_and_complete_qwen_bundle() {
        let qwen=MODELS.iter().find(|m|m.editing).unwrap();
        assert_eq!(qwen.name,crate::ai_generation::QWEN_EDIT);
        assert_eq!(qwen.files.len(),3);
        for category in ["diffusion_models","text_encoders","vae"] {assert!(qwen.files.iter().any(|f|f.directory==category));}
        assert!(MODELS.iter().any(|m|m.keyword=="sd_xl_base" && !m.editing));
        for model in MODELS {for file in model.files {assert!(file.min<=file.size && file.size<=file.max);assert!(file.url.starts_with("https://huggingface.co/"));}}
    }
}
