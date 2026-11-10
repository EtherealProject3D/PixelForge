//! Local-only ComfyUI text-to-image client. No cloud endpoint is accepted.
use serde_json::{json, Value};
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

#[derive(Clone)]
pub struct Request {
    pub checkpoint: String,
    pub prompt: String,
    pub negative: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg: f32,
    pub seed: u64,
    pub source: Option<image::RgbaImage>,
    pub denoise: f32,
    pub mask: Option<Vec<u8>>,
    pub lora: Option<String>,
    pub lora_strength: f32,
}

pub const QWEN_EDIT: &str = "Qwen-Image-Edit 2509 (FP8)";
pub const QWEN_EDIT_2511: &str = "Qwen-Image-Edit 2511 (FP8)";
pub const FLUX_KLEIN: &str = "FLUX.2 Klein 4B (FP8)";
pub const Z_IMAGE: &str = "CyberRealistic Z-Image Turbo v4";
const Z_IMAGE_UNET: &str = "CyberRealistic_zit_v4.0.safetensors";
const QWEN_2511_UNET: &str = "qwen_image_edit_2511_fp8mixed.safetensors";
const FLUX_UNET: &str = "flux-2-klein-4b-fp8.safetensors";
const FLUX_CLIP: &str = "qwen_3_4b.safetensors";
const FLUX_VAE: &str = "flux2-vae.safetensors";
const QWEN_UNET: &str = "qwen_image_edit_2509_fp8_e4m3fn.safetensors";
const QWEN_CLIP: &str = "qwen_2.5_vl_7b_fp8_scaled.safetensors";
const QWEN_VAE: &str = "qwen_image_vae.safetensors";

pub fn is_qwen(model:&str)->bool {model==QWEN_EDIT || model==QWEN_EDIT_2511}
pub fn is_instruction_model(model:&str)->bool {is_qwen(model)||model==FLUX_KLEIN}
pub fn is_flux1_dev(model: &str) -> bool {
    model.rsplit(['/', '\\']).next().is_some_and(|file| {
        let file = file.to_ascii_lowercase();
        file == "flux1-dev-fp8.safetensors" || file == "flux1-dev.safetensors"
    })
}
pub fn ignores_negative(model:&str)->bool {model==FLUX_KLEIN || model==Z_IMAGE || is_flux1_dev(model)}

fn z_image_workflow(prompt:&str,seed:u64,steps:u32,width:u32,height:u32)->Value {
    json!({
        "1":{"class_type":"UNETLoader","inputs":{"unet_name":Z_IMAGE_UNET,"weight_dtype":"default"}},
        "2":{"class_type":"CLIPLoader","inputs":{"clip_name":FLUX_CLIP,"type":"lumina2","device":"default"}},
        "3":{"class_type":"VAELoader","inputs":{"vae_name":"ae.safetensors"}},
        "4":{"class_type":"EmptySD3LatentImage","inputs":{"width":width,"height":height,"batch_size":1}},
        "8":{"class_type":"CLIPTextEncode","inputs":{"clip":["2",0],"text":prompt}},
        "9":{"class_type":"ConditioningZeroOut","inputs":{"conditioning":["8",0]}},
        "10":{"class_type":"ModelSamplingAuraFlow","inputs":{"model":["1",0],"shift":3.0}},
        "5":{"class_type":"KSampler","inputs":{"model":["10",0],"positive":["8",0],"negative":["9",0],"latent_image":["4",0],"seed":seed,"steps":steps,"cfg":1.0,"sampler_name":"res_multistep","scheduler":"simple","denoise":1.0}},
        "6":{"class_type":"VAEDecode","inputs":{"samples":["5",0],"vae":["3",0]}},
        "7":{"class_type":"SaveImage","inputs":{"filename_prefix":"PixelForge_ZImage","images":["6",0]}}
    })
}

fn validate_z_image_nodes(info:&Value)->Result<(),String> {
    for node in ["UNETLoader","CLIPLoader","VAELoader","EmptySD3LatentImage","ModelSamplingAuraFlow","ConditioningZeroOut","CLIPTextEncode","KSampler","VAEDecode","SaveImage"] {
        if info.get(node).is_none() {return Err(format!("Z-Image Turbo needs ComfyUI node {node}. Update ComfyUI from Setup > Dependencies."));}
    }
    for (node,input,value) in [("UNETLoader","unet_name",Z_IMAGE_UNET),("CLIPLoader","clip_name",FLUX_CLIP),("VAELoader","vae_name","ae.safetensors"),("CLIPLoader","type","lumina2"),("KSampler","sampler_name","res_multistep"),("KSampler","scheduler","simple")] {
        if !info.pointer(&format!("/{node}/input/required/{input}/0")).and_then(Value::as_array).is_some_and(|names|names.iter().any(|name|name.as_str()==Some(value))) {
            return Err(format!("Z-Image Turbo needs {input}: {value}. Install the complete bundle in Model Library, refresh models, and update/restart ComfyUI if needed."));
        }
    }
    Ok(())
}
fn qwen_unet(model:&str)->&'static str {if model==QWEN_EDIT_2511 {QWEN_2511_UNET} else {QWEN_UNET}}

fn qwen_workflow(filename: &str, prompt: &str, negative: &str, seed: u64, steps: u32, model:&str) -> Value {
    json!({
        "1":{"class_type":"UNETLoader","inputs":{"unet_name":qwen_unet(model),"weight_dtype":"default"}},
        "2":{"class_type":"CLIPLoader","inputs":{"clip_name":QWEN_CLIP,"type":"qwen_image","device":"default"}},
        "3":{"class_type":"VAELoader","inputs":{"vae_name":QWEN_VAE}},
        "4":{"class_type":"LoadImage","inputs":{"image":filename}},
        "9":{"class_type":"FluxKontextImageScale","inputs":{"image":["4",0]}},
        "10":{"class_type":"TextEncodeQwenImageEditPlus","inputs":{"clip":["2",0],"vae":["3",0],"image1":["9",0],"prompt":prompt}},
        "11":{"class_type":"TextEncodeQwenImageEditPlus","inputs":{"clip":["2",0],"vae":["3",0],"image1":["9",0],"prompt":negative}},
        "12":{"class_type":"ModelSamplingAuraFlow","inputs":{"model":["1",0],"shift":if model==QWEN_EDIT_2511 {3.1} else {3.0}}},
        "13":{"class_type":"CFGNorm","inputs":{"model":["12",0],"strength":1.0,"pre_cfg":false}},
        "8":{"class_type":"VAEEncode","inputs":{"pixels":["9",0],"vae":["3",0]}},
        "5":{"class_type":"KSampler","inputs":{"model":["13",0],"positive":["10",0],"negative":["11",0],"latent_image":["8",0],"seed":seed,"steps":steps,"cfg":4.0,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
        "6":{"class_type":"VAEDecode","inputs":{"samples":["5",0],"vae":["3",0]}},
        "7":{"class_type":"SaveImage","inputs":{"filename_prefix":"PixelForge_QwenEdit","images":["6",0]}}
    })
}

// Qwen chooses a working size internally. Return the edit on the source's
// exact pixel grid; metadata is populated from these final pixels by the caller.
fn restore_edit_dimensions(output: image::RgbaImage, source_size: Option<(u32, u32)>) -> image::RgbaImage {
    match source_size {
        Some((width, height)) if output.dimensions() != (width, height) =>
            image::imageops::resize(&output, width, height, image::imageops::FilterType::Lanczos3),
        _ => output,
    }
}

// Feather inward so protected pixels remain bit-for-bit unchanged.
fn feather_selection(mask:&[u8],width:usize,height:usize,radius:f32) -> Vec<u8> {
    let mut distance:Vec<f32>=mask.iter().map(|&value|if value==0 {0.0} else {f32::INFINITY}).collect();
    for y in 0..height {for x in 0..width {
        let i=y*width+x;
        if x>0 {distance[i]=distance[i].min(distance[i-1]+1.0);}
        if y>0 {distance[i]=distance[i].min(distance[i-width]+1.0);}
    }}
    for y in (0..height).rev() {for x in (0..width).rev() {
        let i=y*width+x;
        if x+1<width {distance[i]=distance[i].min(distance[i+1]+1.0);}
        if y+1<height {distance[i]=distance[i].min(distance[i+width]+1.0);}
    }}
    let deepest=distance.iter().copied().fold(0.0_f32,f32::max);
    let radius=radius.min(deepest).max(1.0);
    mask.iter().zip(distance).map(|(&value,d)| {
        let t=(d/radius).clamp(0.0,1.0);
        (value as f32*t*t*(3.0-2.0*t)).round() as u8
    }).collect()
}

// Preserve exact source pixels outside the selection for every edit model.
fn finish_edit(output:image::RgbaImage, source:Option<&image::RgbaImage>, mask:Option<&[u8]>) -> Result<image::RgbaImage,String> {
    let mut output=restore_edit_dimensions(output,source.map(|image|image.dimensions()));
    if let Some(mask)=mask {
        let source=source.ok_or("A selection edit requires a source image")?;
        if mask.len()!=source.width() as usize*source.height() as usize || !mask.iter().any(|&v|v>0) {
            return Err("Select an area to edit before applying an edit to selection".into());
        }
        let feathered=feather_selection(mask,source.width() as usize,source.height() as usize,12.0);
        for ((result,original),&amount) in output.pixels_mut().zip(source.pixels()).zip(&feathered) {
            for channel in 0..4 {
                result[channel]=((result[channel] as u32*amount as u32+original[channel] as u32*(255-amount as u32)+127)/255) as u8;
            }
        }
    }
    Ok(output)
}

fn validate_qwen_nodes(info: &Value, model:&str) -> Result<(), String> {
    for node in ["UNETLoader", "CLIPLoader", "VAELoader", "FluxKontextImageScale", "TextEncodeQwenImageEditPlus", "ModelSamplingAuraFlow", "CFGNorm"] {
        if info.get(node).is_none() { return Err(format!("Qwen Edit needs ComfyUI node {node}. Update ComfyUI; see docs/QWEN_EDIT_SETUP.md.")); }
    }
    for (node, input, file, folder) in [("UNETLoader","unet_name",qwen_unet(model),"diffusion_models"),("CLIPLoader","clip_name",QWEN_CLIP,"text_encoders"),("VAELoader","vae_name",QWEN_VAE,"vae")] {
        let names = info.pointer(&format!("/{node}/input/required/{input}/0")).and_then(Value::as_array);
        if !names.is_some_and(|names| names.iter().any(|v| v.as_str() == Some(file))) {
            return Err(format!("Qwen Edit file missing: models/{folder}/{file}. Open Generate > Model Library to install Qwen, then restart ComfyUI. No SDXL fallback was used."));
        }
    }
    Ok(())
}

fn flux_workflow(filename:Option<&str>,prompt:&str,seed:u64,steps:u32,width:u32,height:u32)->Value {
    let mut graph=json!({
        "1":{"class_type":"UNETLoader","inputs":{"unet_name":FLUX_UNET,"weight_dtype":"default"}},
        "2":{"class_type":"CLIPLoader","inputs":{"clip_name":FLUX_CLIP,"type":"flux2","device":"default"}},
        "3":{"class_type":"VAELoader","inputs":{"vae_name":FLUX_VAE}},
        "4":{"class_type":"EmptyFlux2LatentImage","inputs":{"width":width,"height":height,"batch_size":1}},
        "5":{"class_type":"SamplerCustomAdvanced","inputs":{"noise":["6",0],"guider":["10",0],"sampler":["15",0],"sigmas":["14",0],"latent_image":["4",0]}},
        "6":{"class_type":"RandomNoise","inputs":{"noise_seed":seed}},
        "7":{"class_type":"SaveImage","inputs":{"filename_prefix":"PixelForge_FluxKlein","images":["12",0]}},
        "8":{"class_type":"CLIPTextEncode","inputs":{"clip":["2",0],"text":prompt}},
        "9":{"class_type":"ConditioningZeroOut","inputs":{"conditioning":["8",0]}},
        "10":{"class_type":"CFGGuider","inputs":{"model":["1",0],"positive":["8",0],"negative":["9",0],"cfg":1.0}},
        "12":{"class_type":"VAEDecode","inputs":{"samples":["5",0],"vae":["3",0]}},
        "14":{"class_type":"Flux2Scheduler","inputs":{"steps":steps,"width":width,"height":height}},
        "15":{"class_type":"KSamplerSelect","inputs":{"sampler_name":"euler"}}
    });
    if let Some(filename)=filename {
        graph["16"]=json!({"class_type":"LoadImage","inputs":{"image":filename}});
        graph["17"]=json!({"class_type":"ImageScaleToTotalPixels","inputs":{"image":["16",0],"upscale_method":"lanczos","megapixels":1.0,"resolution_steps":16}});
        graph["18"]=json!({"class_type":"GetImageSize","inputs":{"image":["17",0]}});
        graph["19"]=json!({"class_type":"VAEEncode","inputs":{"pixels":["17",0],"vae":["3",0]}});
        graph["20"]=json!({"class_type":"ReferenceLatent","inputs":{"conditioning":["8",0],"latent":["19",0]}});
        graph["21"]=json!({"class_type":"ReferenceLatent","inputs":{"conditioning":["9",0],"latent":["19",0]}});
        graph["10"]["inputs"]["positive"]=json!(["20",0]);graph["10"]["inputs"]["negative"]=json!(["21",0]);
        for node in ["4","14"] {graph[node]["inputs"]["width"]=json!(["18",0]);graph[node]["inputs"]["height"]=json!(["18",1]);}
    }
    graph
}
fn validate_flux_nodes(info:&Value,editing:bool)->Result<(),String> {
    let mut required=vec!["UNETLoader","CLIPLoader","VAELoader","EmptyFlux2LatentImage","Flux2Scheduler","KSamplerSelect","RandomNoise","CFGGuider","SamplerCustomAdvanced","ConditioningZeroOut"];
    if editing {required.extend(["ReferenceLatent","ImageScaleToTotalPixels","GetImageSize","VAEEncode","LoadImage"]);}
    for node in required {if info.get(node).is_none() {return Err(format!("FLUX Klein needs ComfyUI node {node}. Update ComfyUI from Setup > Dependencies before using this model."));}}
    for (node,input,file,folder) in [("UNETLoader","unet_name",FLUX_UNET,"diffusion_models"),("CLIPLoader","clip_name",FLUX_CLIP,"text_encoders"),("VAELoader","vae_name",FLUX_VAE,"vae")] {
        if !info.pointer(&format!("/{node}/input/required/{input}/0")).and_then(Value::as_array).is_some_and(|names|names.iter().any(|name|name.as_str()==Some(file))) {
            return Err(format!("FLUX Klein file missing: models/{folder}/{file}. Install its bundle in Model Library and refresh ComfyUI."));
        }
    }
    if !info.pointer("/CLIPLoader/input/required/type/0").and_then(Value::as_array).is_some_and(|types|types.iter().any(|t|t.as_str()==Some("flux2"))) {return Err("ComfyUI needs an update to support the FLUX.2 text encoder".into());}
    Ok(())
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .no_proxy().timeout(Duration::from_secs(25))
        .build().map_err(|e| e.to_string())
}

const BASE: &str = "http://127.0.0.1:8188";

use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static MANAGED: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
fn managed() -> &'static Mutex<Option<Child>> { MANAGED.get_or_init(|| Mutex::new(None)) }

pub const CPU_FALLBACK_HINT: &str = "Try with CPU in the Generate tab or select CPU in Setup > Setup & Diagnostics.";

fn server_ready(processing: &str) -> Result<bool, String> {
    let stats = reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(2)).build()
        .and_then(|client| client.get(format!("{BASE}/system_stats")).send())
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Value>());
    let Ok(stats) = stats else { return Ok(false) };
    let device = stats["devices"].get(0).and_then(|device| device["type"].as_str());
    if (processing == "cpu" && device != Some("cpu"))
        || (processing == "gpu" && matches!(device, None | Some("cpu")))
    {
        return Err("The running ComfyUI server uses a different processing device. Stop it and reconnect to apply the selected device.".into());
    }
    Ok(true)
}

/// Start only a PixelForge-owned local backend.
pub fn ensure_server() -> Result<(), String> {
    let config = crate::backend::Config::load();
    ensure_server_for(&config).map_err(|error| {
        if config.processing == "cpu" { error } else { format!("{error}\n{CPU_FALLBACK_HINT}") }
    })
}

fn ensure_server_for(config: &crate::backend::Config) -> Result<(), String> {
    if server_ready(&config.processing)? { return Ok(()); }
    let mut owned = managed().lock().map_err(|e| e.to_string())?;
    if let Some(child) = owned.as_mut() {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() { *owned = None; }
    }
    if owned.is_none() {
        let folder = config.resolve()?;
        let mut command = Command::new(crate::compatibility::python(&folder).ok_or("Backend Python missing")?);
        command.current_dir(&folder).arg("-s").arg("ComfyUI/main.py");
        if folder.join("python_embeded/python.exe").is_file() { command.arg("--windows-standalone-build"); }
        if config.processing == "cpu" {
            command.args(["--cpu", "--force-fp32"]);
        } else {
            let runtime = crate::compatibility::probe(&folder)?;
            if runtime["cuda"].is_string() {
                crate::compatibility::preflight(&runtime, &config.mode)?;
            } else if config.mode == "legacy" || config.mode == "modern" {
                return Err("Selected NVIDIA backend has no CUDA runtime. Choose a matching package in Setup > Setup & Diagnostics.".into());
            }
            if config.processing == "gpu" {
                crate::compatibility::gpu_preflight(&runtime)?;
            }
            if config.mode == "legacy" || crate::compatibility::selected_device(&runtime)
                .and_then(|device| device["capability"][0].as_u64()).is_some_and(|major| major <= 6)
            {
                command.args(["--force-fp32", "--use-pytorch-cross-attention"]);
            }
        }
        let settings = crate::profile::settings_root().ok_or("APPDATA unavailable")?;
        std::fs::create_dir_all(&settings).map_err(|e|e.to_string())?;
        let mut model_configs = Vec::new();
        let models = crate::model_installer::chosen_models_folder()
            .unwrap_or_else(crate::model_installer::recommended_models_folder);
        if models.is_dir() {
            let config = settings.join("extra-model-paths.yaml");
            let yaml=serde_yaml::to_string(&serde_json::json!({"pixelforge":{"base_path":models,"checkpoints":"checkpoints","diffusion_models":"diffusion_models","text_encoders":"text_encoders","vae":"vae","loras":"loras"}})).map_err(|e|e.to_string())?;
            std::fs::write(&config,yaml).map_err(|e|e.to_string())?;
            model_configs.push(config);
        }
        if let Some(roaming)=std::env::var_os("APPDATA") {
            if let Ok(entries)=std::fs::read_dir(PathBuf::from(roaming).join("Comfy Desktop/instance-model-paths")) {
                let mut files:Vec<_>=entries.flatten().map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e=="yaml")).collect();files.sort();
                if let Some(config)=files.first() {command.arg("--extra-model-paths-config").arg(config);}
            }
        }
        if !model_configs.is_empty() {command.arg("--extra-model-paths-config").args(model_configs);}
        let log = std::fs::File::create(settings.join("backend.log")).map_err(|e|e.to_string())?;
        command.arg("--disable-auto-launch").arg("--listen").arg("127.0.0.1").arg("--port").arg("8188")
            .stdout(log.try_clone().map_err(|e|e.to_string())?).stderr(log).stdin(Stdio::null());
        #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        *owned = Some(command.spawn().map_err(|e| format!("Cannot start ComfyUI backend: {e}"))?);
    }
    drop(owned);
    for _ in 0..90 {
        if server_ready(&config.processing)? { return Ok(()); }
        std::thread::sleep(Duration::from_secs(1));
        let mut owned = managed().lock().map_err(|e| e.to_string())?;
        if let Some(child) = owned.as_mut() {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                *owned = None;
                let path=crate::profile::settings_root().ok_or("APPDATA unavailable")?.join("backend.log");
                let raw=std::fs::read_to_string(&path).unwrap_or_default();
                return Err(format!("Backend exited ({status}). Log: {}\n{}",path.display(),crate::compatibility::actionable(&raw)));
            }
        }
    }
    Err("ComfyUI did not become ready within 90 seconds. Inspect backend.log in the PixelForge profile and run GPU diagnostics in Setup > Setup & Diagnostics.".into())
}

pub fn stop_managed_backend() {
    if let Ok(mut guard) = managed().lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}


pub struct ModelCatalog {
    pub checkpoints: Vec<String>,
    pub loras: Vec<String>,
}

pub fn model_catalog() -> Result<ModelCatalog, String> {
    ensure_server()?;
    let http = client()?;
    let info: Value = http.get(format!("{BASE}/object_info"))
        .send()
        .map_err(|error| format!("Cannot connect to local ComfyUI: {error}"))?
        .error_for_status().map_err(|error| error.to_string())?
        .json().map_err(|error| error.to_string())?;
    catalog_from_info(&info)
}

fn catalog_from_info(info: &Value) -> Result<ModelCatalog, String> {
    let list = |node: &str, field: &str| -> Result<Vec<String>, String> {
        let items = info
            .pointer(&format!("/{node}/input/required/{field}/0"))
            .and_then(Value::as_array)
            .ok_or_else(|| format!("ComfyUI did not return {field} names"))?;
        Ok(items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect())
    };
    let mut checkpoints = list("CheckpointLoaderSimple", "ckpt_name")?;
    if validate_flux_nodes(info, false).is_ok() {
        checkpoints.push(FLUX_KLEIN.to_owned());
    }
    if validate_z_image_nodes(info).is_ok() {
        checkpoints.push(Z_IMAGE.to_owned());
    }
    for model in [QWEN_EDIT, QWEN_EDIT_2511] {
        if validate_qwen_nodes(info, model).is_ok() {
            checkpoints.push(model.to_owned());
        }
    }
    checkpoints.sort_unstable_by_key(|name| name.to_ascii_lowercase());
    checkpoints.dedup();
    Ok(ModelCatalog { checkpoints, loras: list("LoraLoader", "lora_name").unwrap_or_default() })
}

fn configure_flux1_dev(workflow: &mut Value, request: &Request) {
    workflow["4"]["class_type"] = json!("EmptySD3LatentImage");
    workflow["5"]["inputs"]["cfg"] = json!(1.0);
    workflow["5"]["inputs"]["scheduler"] = json!("simple");
    workflow["11"] = json!({"class_type":"FluxGuidance","inputs":{
        "conditioning":["2",0], "guidance": request.cfg
    }});
    workflow["12"] = json!({"class_type":"ConditioningZeroOut","inputs":{"conditioning":["2",0]}});
    workflow["5"]["inputs"]["positive"] = json!(["11", 0]);
    workflow["5"]["inputs"]["negative"] = json!(["12", 0]);
    workflow
        .as_object_mut()
        .expect("workflow is an object")
        .remove("3");
    if let Some(lora) = &request.lora {
        workflow["13"] = json!({"class_type":"LoraLoader","inputs":{
            "model":["1",0], "clip":["1",1], "lora_name":lora,
            "strength_model":request.lora_strength, "strength_clip":request.lora_strength
        }});
        workflow["2"]["inputs"]["clip"] = json!(["13", 1]);
        workflow["5"]["inputs"]["model"] = json!(["13", 0]);
    }
}

struct ProgressStop(Arc<AtomicBool>);
impl Drop for ProgressStop {
    fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); }
}

fn valid_dimensions(width:u32,height:u32)->bool {
    (256..=4096).contains(&width) && (256..=4096).contains(&height) && width%16==0 && height%16==0
}
#[cfg(test)]
mod dimension_tests {
    #[test] fn generator_accepts_finer_grid_and_rejects_invalid_dimensions() {
        for (w,h) in [(1024,576),(1024,672),(1024,688),(288,1024)] {assert!(super::valid_dimensions(w,h));}
        for (w,h) in [(1023,576),(1024,250),(4097,1024)] {assert!(!super::valid_dimensions(w,h));}
    }
}
// A local mask gets its own working canvas so a small correction retains detail.
struct InpaintRegion {
    original: image::RgbaImage,
    x: u32,
    y: u32,
}

fn prepare_inpaint(request: &mut Request) -> Result<Option<InpaintRegion>, String> {
    let Some(mask) = request.mask.as_ref() else {
        return Ok(None);
    };
    let source = request
        .source
        .as_ref()
        .ok_or("Inpainting requires a source image")?;
    let (width, height) = source.dimensions();
    if mask.len() != width as usize * height as usize {
        return Err("Inpainting mask dimensions do not match the source image".into());
    }
    let mask =
        image::GrayImage::from_raw(width, height, mask.clone()).ok_or("Invalid inpainting mask")?;
    let mut bounds = None;
    for (x, y, _) in mask.enumerate_pixels().filter(|(_, _, pixel)| pixel[0] > 0) {
        let [left, top, right, bottom] = bounds.unwrap_or([x, y, x + 1, y + 1]);
        bounds = Some([left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1)]);
    }
    let [left, top, right, bottom] =
        bounds.ok_or("Paint an area to edit before starting inpainting")?;

    if is_instruction_model(&request.checkpoint) || request.checkpoint == Z_IMAGE {
        return Ok(None); // Reference-based editors need the full source context.
    }

    let padding = ((right - left).max(bottom - top) / 4).max(32);
    let crop_axis = |start: u32, end: u32, limit: u32| {
        let size = (end - start)
            .saturating_add(padding.saturating_mul(2))
            .max(256)
            .min(limit);
        let start = start
            .saturating_sub((size - (end - start)) / 2)
            .min(limit - size);
        (start, size)
    };
    let (x, crop_width) = crop_axis(left, right, width);
    let (y, crop_height) = crop_axis(top, bottom, height);
    if (crop_width, crop_height) == (width, height) {
        return Ok(None);
    }

    let target = request.width.max(request.height).min(1024);
    let scale = f64::from(target) / f64::from(crop_width.max(crop_height));
    let dimension = |size| ((f64::from(size) * scale / 16.0).round() as u32 * 16).clamp(256, 1024);
    request.width = dimension(crop_width);
    request.height = dimension(crop_height);
    let patch = image::imageops::crop_imm(source, x, y, crop_width, crop_height).to_image();
    let original_source = request
        .source
        .replace(patch)
        .expect("validated inpainting source");
    request.mask = Some(
        image::imageops::crop_imm(&mask, x, y, crop_width, crop_height)
            .to_image()
            .into_raw(),
    );
    Ok(Some(InpaintRegion {
        original: original_source,
        x,
        y,
    }))
}

pub fn generate(mut request: Request, cancel: Arc<AtomicBool>, progress: Arc<std::sync::atomic::AtomicU32>) -> Result<image::RgbaImage, String> {
    if request.prompt.trim().is_empty() { return Err("Enter a generation prompt".into()); }
    if request.checkpoint.trim().is_empty() { return Err("Select a local checkpoint".into()); }
    if !valid_dimensions(request.width,request.height) {
        return Err("Dimensions must be multiples of 16, between 256 and 4096".into());
    }
    if cancel.load(Ordering::Relaxed) { return Err("Generation cancelled".into()); }
    if !(1..=100).contains(&request.steps) {return Err("Steps must be between 1 and 100".into());}
    let mut region = prepare_inpaint(&mut request)?;
    ensure_server()?;
    let qwen = is_qwen(&request.checkpoint);
    let flux=request.checkpoint==FLUX_KLEIN;
    let z_image=request.checkpoint==Z_IMAGE;
    if z_image {
        if request.source.is_some() {return Err("Z-Image Turbo currently supports new image generation. Choose FLUX or Qwen for edits.".into());}
        let info:Value=client()?.get(format!("{BASE}/object_info")).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?.json().map_err(|e|e.to_string())?;
        validate_z_image_nodes(&info)?;
    }
    if qwen {
        if request.source.is_none() { return Err("Qwen Edit requires an existing image. Use a generation checkpoint to create one first.".into()); }
        let info: Value = client()?.get(format!("{BASE}/object_info")).send().map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json().map_err(|e| e.to_string())?;
        validate_qwen_nodes(&info,&request.checkpoint)?;
    }
    if flux {
        let info:Value=client()?.get(format!("{BASE}/object_info")).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?.json().map_err(|e|e.to_string())?;
        validate_flux_nodes(&info,request.source.is_some())?;
    }
    let mut workflow = json!({
        "1": {"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":request.checkpoint}},
        "2": {"class_type":"CLIPTextEncode","inputs":{"text":request.prompt,"clip":["1",1]}},
        "3": {"class_type":"CLIPTextEncode","inputs":{"text":request.negative,"clip":["1",1]}},
        "4": {"class_type":"EmptyLatentImage","inputs":{"width":request.width,"height":request.height,"batch_size":1}},
        "5": {"class_type":"KSampler","inputs":{
            "seed":request.seed,"steps":request.steps,"cfg":request.cfg,"sampler_name":"euler",
            "scheduler":"normal","denoise":1.0,"model":["1",0],"positive":["2",0],
            "negative":["3",0],"latent_image":["4",0]}},
        "6": {"class_type":"VAEDecode","inputs":{"samples":["5",0],"vae":["1",2]}},
        "7": {"class_type":"SaveImage","inputs":{"filename_prefix":"PixelForge","images":["6",0]}}
    });
    if is_flux1_dev(&request.checkpoint) {
        configure_flux1_dev(&mut workflow, &request);
    } else if request.checkpoint.rsplit(['/', '\\']).next().is_some_and(|file| {
        file.eq_ignore_ascii_case("ponyDiffusionV6XL_v6StartWithThisOne.safetensors")
    }) {
        workflow["11"] = json!({"class_type":"CLIPSetLastLayer","inputs":{"clip":["1",1],"stop_at_clip_layer":-2}});
        workflow["2"]["inputs"]["clip"] = json!(["11",0]);
        workflow["3"]["inputs"]["clip"] = json!(["11",0]);
    }
    if flux {workflow=flux_workflow(None,&request.prompt,request.seed,request.steps,request.width,request.height);}
    if z_image {workflow=z_image_workflow(&request.prompt,request.seed,request.steps,request.width,request.height);}
    let http = client()?;

    if let Some(source) = request.source.as_ref() {
        if !(0.01..=1.0).contains(&request.denoise) {
            return Err("Refinement strength must be between 0.01 and 1.0".into());
        }
        let mut resized = image::imageops::resize(source, request.width, request.height, image::imageops::FilterType::Lanczos3);
        if let Some(mask) = request.mask.as_ref() {
            if mask.len() != (source.width() as usize) * (source.height() as usize) || !mask.iter().any(|&v|v>0) {
                return Err("Paint an area to edit before starting inpainting".into());
            }
            let original_mask = image::GrayImage::from_raw(source.width(),source.height(),mask.clone())
                .ok_or("Invalid inpainting mask dimensions")?;
            // Keep the intact reference for instruction models. ComfyUI's inpaint
            // encoder rounds its input mask, so send binary coverage there and
            // retain the original soft mask for local compositing.
            if !qwen && !flux {
                let resized_mask = image::imageops::resize(
                    &original_mask, request.width, request.height, image::imageops::FilterType::Triangle,
                );
                for (x, y, pixel) in resized.enumerate_pixels_mut() {
                    pixel[3] = if resized_mask.get_pixel(x, y)[0] > 0 { 0 } else { 255 };
                }
            }
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(resized)
            .write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let filename = format!("pixelforge_refine_{}_{}.png",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        let part = reqwest::blocking::multipart::Part::bytes(png.into_inner())
            .file_name(filename).mime_str("image/png").map_err(|e|e.to_string())?;
        let form = reqwest::blocking::multipart::Form::new().part("image",part).text("type","input").text("overwrite","false");
        let uploaded: Value = http.post(format!("{BASE}/upload/image")).multipart(form)
            .send().map_err(|e|format!("ComfyUI image upload failed: {e}"))?
            .error_for_status().map_err(|e|format!("ComfyUI rejected source image: {e}"))?
            .json().map_err(|e|e.to_string())?;
        let stored = uploaded["name"].as_str().ok_or("ComfyUI did not return an uploaded filename")?;
        let subfolder = uploaded["subfolder"].as_str().unwrap_or("");
        if qwen {
            let filename = if subfolder.is_empty() { stored.to_owned() } else { format!("{subfolder}/{stored}") };
            workflow = qwen_workflow(&filename, &request.prompt, &request.negative, request.seed, request.steps,&request.checkpoint);
        } else if flux {
            let filename=if subfolder.is_empty() {stored.to_owned()} else {format!("{subfolder}/{stored}")};
            workflow=flux_workflow(Some(&filename),&request.prompt,request.seed,request.steps,request.width,request.height);
        } else {
        workflow["4"] = json!({"class_type":"LoadImage","inputs":{"image": if subfolder.is_empty(){stored.to_owned()}else{format!("{subfolder}/{stored}")}}});
        workflow["8"] = if request.mask.is_some() {
            json!({"class_type":"VAEEncodeForInpaint","inputs":{"pixels":["4",0],"vae":["1",2],"mask":["4",1],"grow_mask_by":6}})
        } else {
            json!({"class_type":"VAEEncode","inputs":{"pixels":["4",0],"vae":["1",2]}})
        };
        workflow["5"]["inputs"]["latent_image"] = json!(["8",0]);
        // Inpainting requires sufficient noise to replace unwanted objects.
        // Whole-image img2img keeps the user's chosen strength unchanged.
        workflow["5"]["inputs"]["denoise"] = json!(if request.mask.is_some() {
            request.denoise.max(0.75)
        } else {
            request.denoise
        });
        }

    }
    // Each generation has a private websocket client ID; ComfyUI emits actual sampler steps.
    let client_id = format!("pixelforge-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos());
    let stop_progress = Arc::new(AtomicBool::new(false));
    let stop_ws = stop_progress.clone();
    let step_value = progress.clone();
    let ws_client = client_id.clone();
    std::thread::spawn(move || {
        if let Ok((mut socket, _)) = tungstenite::connect(format!("ws://127.0.0.1:8188/ws?clientId={ws_client}")) {
            if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            }
            while !stop_ws.load(Ordering::Relaxed) {
                match socket.read() {
                    Ok(tungstenite::Message::Text(body)) => {
                        if let Ok(event) = serde_json::from_str::<Value>(&body) {
                            if event["type"] == "progress" {
                                if let (Some(value), Some(max)) = (
                                    event.pointer("/data/value").and_then(Value::as_u64),
                                    event.pointer("/data/max").and_then(Value::as_u64)) {
                                    if max > 0 {
                                        step_value.store(value.min(max) as u32, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => break,
                }
            }
        }
    });
    let response: Value = http.post(format!("{BASE}/prompt")).json(&json!({"prompt":workflow,"client_id":client_id}))
        .send().map_err(|e| format!("Cannot submit ComfyUI prompt: {e}"))?
        .error_for_status().map_err(|e| format!("ComfyUI rejected workflow: {e}"))?
        .json().map_err(|e| e.to_string())?;
    let id = response.get("prompt_id").and_then(Value::as_str)
        .ok_or_else(|| format!("ComfyUI did not accept the prompt: {response}"))?.to_owned();
    let _progress_guard = ProgressStop(stop_progress);
    let timeout_minutes = if crate::backend::Config::load().processing == "cpu" { 60 } else { 15 };
    let deadline = Instant::now() + Duration::from_secs(timeout_minutes * 60);
    loop {
        if cancel.load(Ordering::Relaxed) {
            // Cancel only this queued prompt; do not interrupt unrelated ComfyUI jobs.
            let _ = http.post(format!("{BASE}/queue")).json(&json!({"delete":[id]})).send();
            return Err("Generation cancelled".into());
        }
        if Instant::now() >= deadline { return Err(format!("ComfyUI generation timed out after {timeout_minutes} minutes")); }
        let history: Value = http.get(format!("{BASE}/history/{id}"))
            .send().map_err(|e| e.to_string())?
            .error_for_status().map_err(|e| e.to_string())?
            .json().map_err(|e| e.to_string())?;
        if let Some(job) = history.get(&id) {
            if job.pointer("/status/status_str").and_then(Value::as_str) == Some("error") {
                return Err(crate::compatibility::actionable(&format!("ComfyUI generation failed: {}", job["status"])));
            }
            if let Some(images) = job.pointer("/outputs/7/images").and_then(Value::as_array) {
                if let Some(first) = images.first() {
                    let filename = first["filename"].as_str().ok_or("Missing output filename")?;
                    let subfolder = first["subfolder"].as_str().unwrap_or("");
                    let kind = first["type"].as_str().unwrap_or("output");
                    let bytes = http.get(format!("{BASE}/view"))
                        .query(&[("filename",filename),("subfolder",subfolder),("type",kind)])
                        .send().map_err(|e| e.to_string())?
                        .error_for_status().map_err(|e| e.to_string())?
                        .bytes().map_err(|e| e.to_string())?;
                    let output = image::load_from_memory(&bytes).map(|img| img.to_rgba8())
                        .map_err(|e| format!("Cannot decode generated image: {e}"))?;
                    let edited = finish_edit(output, request.source.as_ref(), request.mask.as_deref())?;
                    if let Some(mut region) = region.take() {
                        image::imageops::replace(&mut region.original, &edited, i64::from(region.x), i64::from(region.y));
                        return Ok(region.original);
                    }
                    return Ok(edited);
                }
            }
            if job.get("outputs").is_some() { return Err("ComfyUI finished without a saved image".into()); }
        }
        std::thread::sleep(Duration::from_millis(750));
    }
}

#[cfg(test)]
mod qwen_tests {
    use super::*;
    #[test]
    fn qwen_output_returns_to_exact_source_resolution() {
        let output = image::RgbaImage::from_pixel(64, 64, image::Rgba([30, 80, 120, 255]));
        let restored = restore_edit_dimensions(output, Some((101, 73)));
        assert_eq!(restored.dimensions(), (101, 73));
        assert_eq!(*restored.get_pixel(50, 30), image::Rgba([30, 80, 120, 255]));
    }
    #[test]
    fn matching_size_and_non_qwen_results_keep_pixels() {
        let output = image::RgbaImage::from_pixel(7, 5, image::Rgba([1, 2, 3, 4]));
        assert_eq!(restore_edit_dimensions(output.clone(), None), output);
        assert_eq!(restore_edit_dimensions(output.clone(), Some((7, 5))), output);
    }
    #[test]
    fn instruction_editor_uses_image_conditioning_and_dedicated_loaders() {
        let w = qwen_workflow("source.png", "Make the hat blue", "blurry", 42, 7,QWEN_EDIT);
        assert_eq!(w["1"]["class_type"], "UNETLoader");
        assert_eq!(w["10"]["inputs"]["prompt"], "Make the hat blue");
        assert_eq!(w["10"]["inputs"]["image1"], json!(["9",0]));
        assert_eq!(w["5"]["inputs"]["seed"], 42);
        assert_eq!(w["5"]["inputs"]["steps"], 7);
        assert_eq!(w["11"]["inputs"]["prompt"], "blurry");
        assert_eq!(w["7"]["class_type"], "SaveImage");
        assert!(!w.as_object().unwrap().values().any(|n| n["class_type"] == "CheckpointLoaderSimple"));
    }
    #[test]
    fn setup_check_requires_nodes_and_all_three_models() {
        assert!(validate_qwen_nodes(&json!({}),QWEN_EDIT).unwrap_err().contains("Update ComfyUI"));
        let mut info = json!({});
        for node in ["UNETLoader","CLIPLoader","VAELoader","FluxKontextImageScale","TextEncodeQwenImageEditPlus","ModelSamplingAuraFlow","CFGNorm"] { info[node] = json!({}); }
        assert!(validate_qwen_nodes(&info,QWEN_EDIT).unwrap_err().contains(QWEN_UNET));
        for (node,input,file) in [("UNETLoader","unet_name",QWEN_UNET),("CLIPLoader","clip_name",QWEN_CLIP),("VAELoader","vae_name",QWEN_VAE)] { info[node] = json!({"input":{"required":{input:[[file]]}}}); }
        assert!(validate_qwen_nodes(&info,QWEN_EDIT).is_ok());
    }
}

#[cfg(test)]
mod local_model_tests {
    use super::*;
    #[test]
    fn z_image_graph_uses_its_own_loaders_and_distilled_sampling() {
        let graph=z_image_workflow("portrait",123,9,1024,768);
        assert_eq!(graph["1"]["inputs"]["unet_name"],Z_IMAGE_UNET);
        assert_eq!(graph["2"]["inputs"]["type"],"lumina2");
        assert_eq!(graph["3"]["inputs"]["vae_name"],"ae.safetensors");
        assert_eq!(graph["4"]["class_type"],"EmptySD3LatentImage");
        assert_eq!(graph["4"]["inputs"]["width"],1024);
        assert_eq!(graph["4"]["inputs"]["height"],768);
        assert_eq!(graph["5"]["inputs"]["model"],json!(["10",0]));
        assert_eq!(graph["5"]["inputs"]["negative"],json!(["9",0]));
        assert_eq!(graph["5"]["inputs"]["cfg"],1.0);
        assert_eq!(graph["5"]["inputs"]["steps"],9);
        assert_eq!(graph["5"]["inputs"]["seed"],123);
        assert_eq!(graph["9"]["class_type"],"ConditioningZeroOut");
        assert!(!is_instruction_model(Z_IMAGE));
        assert!(ignores_negative(Z_IMAGE));
    }
    #[test]
    fn z_image_validation_detects_missing_files_and_incompatible_backend() {
        assert!(validate_z_image_nodes(&json!({})).unwrap_err().contains("Update ComfyUI"));
        let mut info=json!({});
        for node in ["UNETLoader","CLIPLoader","VAELoader","EmptySD3LatentImage","ModelSamplingAuraFlow","ConditioningZeroOut","CLIPTextEncode","KSampler","VAEDecode","SaveImage"] {info[node]=json!({"input":{"required":{}}});}
        for (node,input,value) in [("UNETLoader","unet_name",Z_IMAGE_UNET),("CLIPLoader","clip_name",FLUX_CLIP),("VAELoader","vae_name","ae.safetensors"),("CLIPLoader","type","lumina2"),("KSampler","sampler_name","res_multistep"),("KSampler","scheduler","simple")] {info[node]["input"]["required"][input]=json!([[value]]);}
        assert!(validate_z_image_nodes(&info).is_ok());
        info["VAELoader"]["input"]["required"]["vae_name"]=json!([[FLUX_VAE]]);
        assert!(validate_z_image_nodes(&info).unwrap_err().contains("ae.safetensors"));
    }
    #[test]
    fn qwen_versions_route_to_their_own_weights_and_sampling_shift() {
        for (model,file,shift) in [(QWEN_EDIT,QWEN_UNET,3.0),(QWEN_EDIT_2511,QWEN_2511_UNET,3.1)] {
            let graph=qwen_workflow("source.png","Change the hat","",17,40,model);
            assert_eq!(graph["1"]["inputs"]["unet_name"],file);
            assert_eq!(graph["12"]["inputs"]["shift"],shift);
            assert_eq!(graph["5"]["inputs"]["steps"],40);
            assert!(is_qwen(model));
        }
        assert!(!is_qwen(FLUX_KLEIN));assert!(is_instruction_model(FLUX_KLEIN));
    }
    #[test]
    fn flux_generation_and_edit_use_flux_latents_reference_conditioning_and_four_steps() {
        for editing in [false,true] {
            let graph=flux_workflow(editing.then_some("source.png"),"A blue car",42,4,1024,768);
            assert_eq!(graph["4"]["class_type"],"EmptyFlux2LatentImage");
            assert_eq!(graph["2"]["inputs"]["type"],"flux2");
            assert_eq!(graph["10"]["inputs"]["cfg"],1.0);
            assert_eq!(graph["14"]["inputs"]["steps"],4);
            assert_eq!(graph["7"]["class_type"],"SaveImage");
            assert_eq!(graph.get("20").is_some(),editing);
            if editing {
                assert_eq!(graph["10"]["inputs"]["positive"],json!(["20",0]));
                assert_eq!(graph["20"]["class_type"],"ReferenceLatent");
                assert_eq!(graph["17"]["inputs"]["resolution_steps"],16);
                assert_eq!(graph["14"]["inputs"]["width"],json!(["18",0]));
            } else {assert_eq!(graph["4"]["inputs"]["width"],1024);}
            for node in graph.as_object().unwrap().values() {
                assert_ne!(node["class_type"],"CheckpointLoaderSimple");
                for input in node["inputs"].as_object().unwrap().values() {
                    if let Some(link)=input.as_array() {if let Some(id)=link.first().and_then(Value::as_str) {assert!(graph.get(id).is_some(),"Missing node {id}");}}
                }
            }
        }
    }
    #[test]
    fn model_catalog_uses_backend_choices_and_omits_incomplete_bundles() {
        let mut info = json!({
            "CheckpointLoaderSimple": {"input":{"required":{"ckpt_name":[["nested/local.safetensors","nested/local.safetensors"]]}}},
            "LoraLoader": {"input":{"required":{"lora_name":[["local-style.safetensors"]]}}},
        });
        let catalog = catalog_from_info(&info).unwrap();
        assert_eq!(catalog.checkpoints, ["nested/local.safetensors"]);
        assert_eq!(catalog.loras, ["local-style.safetensors"]);
        for node in ["UNETLoader","CLIPLoader","VAELoader","EmptyFlux2LatentImage","Flux2Scheduler",
            "KSamplerSelect","RandomNoise","CFGGuider","SamplerCustomAdvanced","ConditioningZeroOut"] {
            info[node] = json!({});
        }
        for (node, input, file) in [("UNETLoader","unet_name",FLUX_UNET),("CLIPLoader","clip_name",FLUX_CLIP),("VAELoader","vae_name",FLUX_VAE)] {
            info[node]["input"]["required"][input] = json!([[file]]);
        }
        info["CLIPLoader"]["input"]["required"]["type"] = json!([["flux2"]]);
        let catalog = catalog_from_info(&info).unwrap();
        assert!(catalog.checkpoints.iter().any(|model| model == FLUX_KLEIN));
        assert!(!catalog.checkpoints.iter().any(|model| model == Z_IMAGE));
    }

    #[test]
    fn setup_validation_requires_exact_bundle_and_flux2_encoder_support() {
        let graph=flux_workflow(Some("x.png"),"edit",1,4,1024,1024);
        let mut info=json!({});for node in graph.as_object().unwrap().values() {info[node["class_type"].as_str().unwrap()]=json!({});}
        for (node,input,file) in [("UNETLoader","unet_name",FLUX_UNET),("CLIPLoader","clip_name",FLUX_CLIP),("VAELoader","vae_name",FLUX_VAE)] {info[node]=json!({"input":{"required":{input:[[file]]}}});}
        assert!(validate_flux_nodes(&info,true).unwrap_err().contains("text encoder"));
        info["CLIPLoader"]["input"]["required"]["type"]=json!([["flux2"]]);
        assert!(validate_flux_nodes(&info,true).is_ok());
        info.as_object_mut().unwrap().remove("ReferenceLatent");
        assert!(validate_flux_nodes(&info,true).unwrap_err().contains("ReferenceLatent"));
        assert!(validate_flux_nodes(&info,false).is_ok());
    }
}

#[cfg(test)]
mod selection_edit_tests {
    use super::*;
    fn inpaint_fixture(checkpoint: &str) -> Request {
        let mut source = image::RgbaImage::from_pixel(640, 384, image::Rgba([135, 185, 220, 255]));
        let mut mask = vec![0; 640 * 384];
        for y in 82..150 {
            for x in 42..104 {
                source.put_pixel(x, y, image::Rgba([225, 35, 25, 255]));
                mask[y as usize * 640 + x as usize] = 255;
            }
        }
        Request {
            checkpoint: checkpoint.into(),
            prompt: "A clean light blue painted surface, even color and natural daylight".into(),
            negative: "red object, red patch".into(),
            width: 640,
            height: 384,
            steps: 14,
            cfg: 5.0,
            seed: 42,
            source: Some(source),
            denoise: 0.95,
            mask: Some(mask),
            lora: None,
            lora_strength: 1.0,
        }
    }

    #[test]
    fn small_inpaint_mask_gets_a_padded_crop_and_valid_working_dimensions() {
        let mut request = inpaint_fixture("sd_xl_base_1.0.safetensors");
        let region = prepare_inpaint(&mut request).unwrap().unwrap();
        let patch = request.source.as_ref().unwrap();
        assert_eq!(region.original.dimensions(), (640, 384));
        assert_eq!(patch.dimensions(), (256, 256));
        assert_eq!(
            request
                .mask
                .as_ref()
                .unwrap()
                .iter()
                .filter(|&&value| value > 0)
                .count(),
            62 * 68
        );
        assert!(valid_dimensions(request.width, request.height));
        assert_eq!((request.width, request.height), (640, 640));

        let mut reference_edit = inpaint_fixture(FLUX_KLEIN);
        assert!(prepare_inpaint(&mut reference_edit).unwrap().is_none());
        assert_eq!(reference_edit.source.unwrap().dimensions(), (640, 384));
    }

    #[test]
    fn flux1_routes_local_lora_and_distilled_guidance_through_sampler() {
        let mut request = inpaint_fixture("flux1-dev-fp8.safetensors");
        request.cfg = 3.5;
        request.lora = Some("style.safetensors".into());
        request.lora_strength = 0.7;
        let mut workflow = json!({"1":{},"2":{"inputs":{}},"3":{},"4":{},"5":{"inputs":{}}});
        configure_flux1_dev(&mut workflow, &request);
        assert_eq!(workflow["4"]["class_type"], "EmptySD3LatentImage");
        assert_eq!(workflow["5"]["inputs"]["cfg"], 1.0);
        assert_eq!(workflow["11"]["inputs"]["guidance"], 3.5);
        assert_eq!(workflow["13"]["inputs"]["lora_name"], "style.safetensors");
        assert_eq!(workflow["2"]["inputs"]["clip"], json!(["13", 1]));
        assert_eq!(workflow["5"]["inputs"]["model"], json!(["13", 0]));
        assert!(workflow.get("3").is_none());
    }

    #[test]
    #[ignore = "Requires a local ComfyUI backend, an installed SDXL Base checkpoint, and GPU time"]
    fn local_inpaint_preserves_every_pixel_outside_the_mask() {
        let request = inpaint_fixture("sd_xl_base_1.0.safetensors");
        let source = request.source.clone().unwrap();
        let mask = request.mask.clone().unwrap();
        let result = generate(
            request,
            Arc::new(AtomicBool::new(false)),
            Arc::new(std::sync::atomic::AtomicU32::new(0)),
        );
        stop_managed_backend();
        let edited = result.unwrap();
        assert_eq!(edited.dimensions(), source.dimensions());
        let mut changed = 0;
        for ((before, after), selected) in source.pixels().zip(edited.pixels()).zip(mask) {
            if selected == 0 {
                assert_eq!(before, after);
            } else if before != after {
                changed += 1;
            }
        }
        assert!(changed > 100, "expected the selected object to change");
    }

    #[test]
    fn selection_composites_exact_originals_and_feathered_edges_at_source_size() {
        let source=image::RgbaImage::from_pixel(3,1,image::Rgba([20,40,60,80]));
        let generated=image::RgbaImage::from_pixel(6,2,image::Rgba([220,240,160,180]));
        let result=finish_edit(generated,Some(&source),Some(&[0,128,255])).unwrap();
        assert_eq!(result.dimensions(),source.dimensions());
        assert_eq!(result.get_pixel(0,0),source.get_pixel(0,0));
        assert_eq!(*result.get_pixel(1,0),image::Rgba([70,90,85,105]));
        assert_eq!(*result.get_pixel(2,0),image::Rgba([220,240,160,180]));
    }
    #[test]
    fn hard_selection_fades_inward_and_never_changes_protected_pixels() {
        let source=image::RgbaImage::from_pixel(40,1,image::Rgba([0,0,0,80]));
        let generated=image::RgbaImage::from_pixel(40,1,image::Rgba([240,240,240,200]));
        let mut mask=vec![0;40];mask[4..36].fill(255);
        let result=finish_edit(generated,Some(&source),Some(&mask)).unwrap();
        for x in [0,1,2,3,36,37,38,39] {assert_eq!(result.get_pixel(x,0),source.get_pixel(x,0));}
        let edge=result.get_pixel(4,0)[0];
        let middle=result.get_pixel(8,0)[0];
        let center=result.get_pixel(20,0)[0];
        assert!(edge>0 && edge<middle && middle<center);
        assert_eq!(center,240);
        assert_eq!(feather_selection(&[255;4],2,2,12.0),vec![255;4]);
    }

    #[test]
    fn selection_requires_nonempty_matching_mask_and_source() {
        let source=image::RgbaImage::new(2,1);
        assert!(finish_edit(source.clone(),Some(&source),Some(&[0,0])).is_err());
        assert!(finish_edit(source.clone(),Some(&source),Some(&[255])).is_err());
        assert!(finish_edit(source.clone(),None,Some(&[255,255])).is_err());
        assert_eq!(finish_edit(source.clone(),Some(&source),None).unwrap(),source);
    }
}
