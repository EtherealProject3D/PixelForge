//! Shared curated model-library installer for SDXL checkpoints and Qwen bundles.
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::Duration};

pub struct ModelFile {
    pub directory: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub min: u64,
    pub max: u64,
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
pub const MODELS: &[Model] = &[
    Model {name:"CyberRealistic Pony v18 CoreShift fp16",keyword:"cyberrealisticpony",description:"Pony XL character realism; 30+ steps, CFG 5",page:"https://huggingface.co/cyberdelia/CyberRealisticPony",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"CyberRealisticPony_V18.0_F16.safetensors",url:"https://huggingface.co/cyberdelia/CyberRealisticPony/resolve/main/CyberRealisticPony_V18.0_F16.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000}]},
    Model {name:"RealVisXL V5 fp16",keyword:"realvis",description:"Natural photography, portraits and landscapes",page:"https://huggingface.co/SG161222/RealVisXL_V5.0",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"RealVisXL_V5.0_fp16.safetensors",url:"https://huggingface.co/SG161222/RealVisXL_V5.0/resolve/main/RealVisXL_V5.0_fp16.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000}]},
    Model {name:"Juggernaut XL v9",keyword:"juggernaut",description:"Detailed photography and cinematic imagery",page:"https://huggingface.co/RunDiffusion/Juggernaut-XL-v9",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",url:"https://huggingface.co/RunDiffusion/Juggernaut-XL-v9/resolve/main/Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors",size:7110000000,min:5_000_000_000,max:12_000_000_000}]},
    Model {name:"SDXL Base 1.0",keyword:"sd_xl_base",description:"General-purpose generation and illustration",page:"https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0",editing:false,generation:true,
        files:&[ModelFile {directory:"checkpoints",file:"sd_xl_base_1.0.safetensors",url:"https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/main/sd_xl_base_1.0.safetensors",size:6940000000,min:5_000_000_000,max:12_000_000_000}]},
    Model {name:"Qwen-Image-Edit 2509 (FP8)",keyword:"qwen",description:"Instruction-based image editing. Installs all three required files (about 30 GB).",page:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit",editing:true,generation:false,
        files:&[
            ModelFile {directory:"diffusion_models",file:"qwen_image_edit_2509_fp8_e4m3fn.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit/resolve/main/split_files/diffusion_models/qwen_image_edit_2509_fp8_e4m3fn.safetensors",size:20_500_000_000,min:18_000_000_000,max:24_000_000_000},
            ModelFile {directory:"text_encoders",file:"qwen_2.5_vl_7b_fp8_scaled.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors",size:9_400_000_000,min:8_000_000_000,max:12_000_000_000},
            ModelFile {directory:"vae",file:"qwen_image_vae.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/vae/qwen_image_vae.safetensors",size:250_000_000,min:200_000_000,max:500_000_000},
        ]},
    Model {name:"Qwen-Image-Edit 2511 (FP8)",keyword:"qwen2511",description:"Newer instruction editor with improved consistency. About 20.5 GB extra when the existing Qwen encoder/VAE are present.",page:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit",editing:true,generation:false,
        files:&[
            ModelFile {directory:"diffusion_models",file:"qwen_image_edit_2511_fp8mixed.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image-Edit/resolve/main/split_files/diffusion_models/qwen_image_edit_2511_fp8mixed.safetensors",size:20_600_000_000,min:18_000_000_000,max:24_000_000_000},
            ModelFile {directory:"text_encoders",file:"qwen_2.5_vl_7b_fp8_scaled.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors",size:9_400_000_000,min:8_000_000_000,max:12_000_000_000},
            ModelFile {directory:"vae",file:"qwen_image_vae.safetensors",url:"https://huggingface.co/Comfy-Org/Qwen-Image/resolve/main/split_files/vae/qwen_image_vae.safetensors",size:250_000_000,min:200_000_000,max:500_000_000},
        ]},
    Model {name:"FLUX.2 Klein 4B (FP8)",keyword:"fluxklein4b",description:"Local generation and instruction editing. Distilled 4-step model; about 12.5 GB total. Uses memory offloading when needed.",page:"https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8",editing:true,generation:true,
        files:&[
            ModelFile {directory:"diffusion_models",file:"flux-2-klein-4b-fp8.safetensors",url:"https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8/resolve/main/flux-2-klein-4b-fp8.safetensors",size:4_100_000_000,min:3_500_000_000,max:5_000_000_000},
            ModelFile {directory:"text_encoders",file:"qwen_3_4b.safetensors",url:"https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/text_encoders/qwen_3_4b.safetensors",size:8_100_000_000,min:7_500_000_000,max:9_000_000_000},
            ModelFile {directory:"vae",file:"flux2-vae.safetensors",url:"https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/vae/flux2-vae.safetensors",size:340_000_000,min:300_000_000,max:450_000_000},
        ]},
];

fn valid_file(file:&ModelFile,path:&Path) -> bool {
    fs::metadata(path).is_ok_and(|m|m.is_file() && (file.min..=file.max).contains(&m.len()))
}
pub fn installed_files(model:&Model,root:&Path) -> usize {
    model.files.iter().filter(|file|valid_file(file,&root.join(file.directory).join(file.file))).count()
}

pub fn install(index:usize,root:PathBuf,progress:Arc<AtomicU64>,total:Arc<AtomicU64>,cancel:Arc<AtomicBool>) -> Result<PathBuf,String> {
    install_model(MODELS.get(index).ok_or("Unknown model")?,root,progress,total,cancel)
}

fn install_model(model:&Model,root:PathBuf,progress:Arc<AtomicU64>,total:Arc<AtomicU64>,cancel:Arc<AtomicBool>) -> Result<PathBuf,String> {
    if !root.is_dir() {return Err("ComfyUI models folder does not exist".into());}
    let mut missing=Vec::new();
    for file in model.files {
        let path=root.join(file.directory).join(file.file);
        if path.exists() {
            if !valid_file(file,&path) {return Err(format!("Existing model file has an unexpected size; inspect it before retrying: {}",path.display()));}
        } else {
            if path.with_extension("safetensors.part").exists() {return Err(format!("Unfinished download exists: {}. Move or delete it before retrying.",path.display()));}
            missing.push(file);
        }
    }
    let expected:u64=missing.iter().map(|file|file.size).sum();
    progress.store(0,Ordering::Relaxed);total.store(expected,Ordering::Relaxed);
    let free=fs2::available_space(&root).map_err(|e|format!("Cannot check free disk space: {e}"))?;
    if free<expected+1_000_000_000 {return Err("Not enough free space (missing model files plus 1 GB safety margin)".into());}
    // Large bundles can take hours. Connection establishment remains bounded.
    let client=reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(24*3600)).build().map_err(|e|e.to_string())?;
    let mut completed=0;
    for file in missing {
        if cancel.load(Ordering::Relaxed) {return Err("Download cancelled".into());}
        let folder=root.join(file.directory);
        fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
        let destination=folder.join(file.file);
        let partial=folder.join(format!("{}.part",file.file));
        let mut response=client.get(file.url).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?;
        let length=response.content_length().ok_or("Server did not provide a file size")?;
        if !(file.min..=file.max).contains(&length) {return Err(format!("Unexpected size for {}: {length} bytes",file.file));}
        let available=fs2::available_space(&folder).map_err(|e|e.to_string())?;
        if available<length+1_000_000_000 {return Err("Insufficient disk space for download".into());}
        // Replace the estimate for this file with the server's actual size.
        let previous=total.load(Ordering::Relaxed);
        total.store(previous-file.size+length,Ordering::Relaxed);
        let mut output=OpenOptions::new().write(true).create_new(true).open(&partial).map_err(|e|e.to_string())?;
        let result=(|| -> Result<(),String> {
            let mut buffer=[0u8;256*1024];let mut written=0;
            loop {
                if cancel.load(Ordering::Relaxed) {return Err("Download cancelled".into());}
                let count=response.read(&mut buffer).map_err(|e|e.to_string())?;
                if count==0 {break;}
                written+=count as u64;
                if written>length {return Err("Download exceeded expected file size".into());}
                output.write_all(&buffer[..count]).map_err(|e|e.to_string())?;
                progress.store(completed+written,Ordering::Relaxed);
            }
            output.sync_all().map_err(|e|e.to_string())?;
            if written!=length {return Err(format!("Incomplete download: {written} of {length} bytes"));}
            if cancel.load(Ordering::Relaxed) {return Err("Download cancelled".into());}
            Ok(())
        })();
        drop(output);
        if let Err(error)=result {let _=fs::remove_file(&partial);return Err(error);}
        publish_download(&partial,&destination)?;
        completed+=length;
    }
    Ok(root)
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
    let mut candidates:Vec<PathBuf>=model.files.iter().filter(|f|!model.editing || include_shared || f.directory=="diffusion_models")
        .map(|f|root.join(f.directory).join(f.file)).collect();
    if !model.editing {
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

pub fn detect_models_folder(known:&[String]) -> Option<PathBuf> {
    detect_checkpoint_folder(known).and_then(|p|p.parent().map(Path::to_path_buf)).or_else(|| {
        crate::dependencies::active_path(crate::dependencies::Component::Comfy)?;
        let root=crate::dependencies::root().join("models");
        fs::create_dir_all(root.join("checkpoints")).ok()?;
        Some(root)
    })
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
            ModelFile {directory:"vae",file:"existing.safetensors",url,size:4,min:4,max:4},
            ModelFile {directory:"diffusion_models",file:"new.safetensors",url,size:4,min:4,max:4},
            ModelFile {directory:"text_encoders",file:"new.safetensors",url,size:4,min:4,max:4},
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
            ModelFile {directory:"vae",file:"complete.safetensors",url,size:4,min:4,max:4},
            ModelFile {directory:"vae",file:"interrupted.safetensors",url,size:4,min:4,max:4},
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
    fn cancelled_install_never_downloads_and_invalid_existing_file_is_preserved() {
        let root=fixture();
        let files=Box::leak(Box::new([ModelFile {directory:"vae",file:"test.safetensors",url:"http://127.0.0.1:1/no-request",size:4,min:4,max:4}]));
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
        assert!(removal_plan(0,&root,Some("../../outside.safetensors"),false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removal_uses_exact_detected_checkpoint_and_reports_disk_bytes() {
        let root=fixture();fs::create_dir(root.join("checkpoints")).unwrap();
        fs::write(root.join("checkpoints/alternate.safetensors"),b"abcdef").unwrap();
        assert_eq!(remove_model_files(0,&root,Some("alternate.safetensors"),false).unwrap(),6);
        assert!(!root.join("checkpoints/alternate.safetensors").exists());fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn new_models_share_only_the_correct_companion_files() {
        let qwen=MODELS.iter().find(|m|m.name==crate::ai_generation::QWEN_EDIT).unwrap();
        let newer=MODELS.iter().find(|m|m.name==crate::ai_generation::QWEN_EDIT_2511).unwrap();
        assert_ne!(qwen.files[0].file,newer.files[0].file);
        assert_eq!(qwen.files[1].file,newer.files[1].file);assert_eq!(qwen.files[2].file,newer.files[2].file);
        let flux=MODELS.iter().find(|m|m.name==crate::ai_generation::FLUX_KLEIN).unwrap();
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
