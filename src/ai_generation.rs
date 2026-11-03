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
}

pub const QWEN_EDIT: &str = "Qwen-Image-Edit 2509 (FP8)";
pub const QWEN_EDIT_2511: &str = "Qwen-Image-Edit 2511 (FP8)";
pub const FLUX_KLEIN: &str = "FLUX.2 Klein 4B (FP8)";
const QWEN_2511_UNET: &str = "qwen_image_edit_2511_fp8mixed.safetensors";
const FLUX_UNET: &str = "flux-2-klein-4b-fp8.safetensors";
const FLUX_CLIP: &str = "qwen_3_4b.safetensors";
const FLUX_VAE: &str = "flux2-vae.safetensors";
const QWEN_UNET: &str = "qwen_image_edit_2509_fp8_e4m3fn.safetensors";
const QWEN_CLIP: &str = "qwen_2.5_vl_7b_fp8_scaled.safetensors";
const QWEN_VAE: &str = "qwen_image_vae.safetensors";

pub fn is_qwen(model:&str)->bool {model==QWEN_EDIT || model==QWEN_EDIT_2511}
pub fn is_instruction_model(model:&str)->bool {is_qwen(model)||model==FLUX_KLEIN}
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
    for node in required {if info.get(node).is_none() {return Err(format!("FLUX Klein needs ComfyUI node {node}. Update ComfyUI from Dependencies before using this model."));}}
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
        .timeout(Duration::from_secs(25))
        .build().map_err(|e| e.to_string())
}

const BASE: &str = "http://127.0.0.1:8188";

use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static MANAGED: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
fn managed() -> &'static Mutex<Option<Child>> { MANAGED.get_or_init(|| Mutex::new(None)) }

fn server_ready() -> bool {
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build()
        .and_then(|c| c.get(format!("{BASE}/system_stats")).send())
        .map(|r| r.status().is_success()).unwrap_or(false)
}

/// Start only a PixelForge-owned local backend. Never terminate a server we did not start.
pub fn ensure_server() -> Result<(), String> {
    if server_ready() { return Ok(()); }
    let mut owned = managed().lock().map_err(|e| e.to_string())?;
    if let Some(child) = owned.as_mut() {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() { *owned = None; }
    }
    if owned.is_none() {
        let managed=crate::dependencies::active_path(crate::dependencies::Component::Comfy);
        let mut command=if let Some(folder)=managed {
            let mut command=Command::new(folder.join("python_embeded/python.exe"));
            command.current_dir(&folder).arg("-s").arg("ComfyUI/main.py").arg("--windows-standalone-build");
            let models=crate::dependencies::root().join("models");
            std::fs::create_dir_all(&models).map_err(|e|e.to_string())?;
            let config=crate::dependencies::root().join("extra-model-paths.yaml");
            let yaml=serde_yaml::to_string(&serde_json::json!({"pixelforge":{"base_path":models,"checkpoints":"checkpoints","diffusion_models":"diffusion_models","text_encoders":"text_encoders","vae":"vae"}})).map_err(|e|e.to_string())?;
            std::fs::write(&config,yaml).map_err(|e|e.to_string())?;
            command.arg("--extra-model-paths-config").arg(config);
            // Reuse the Desktop model paths as well when present.
            if let Some(roaming)=std::env::var_os("APPDATA") {
                if let Ok(entries)=std::fs::read_dir(PathBuf::from(roaming).join("Comfy Desktop/instance-model-paths")) {
                    let mut files:Vec<_>=entries.flatten().map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e=="yaml")).collect();files.sort();
                    if let Some(config)=files.first() {command.arg(config);}
                }
            }
            command
        } else {
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
        let root = PathBuf::from(local).join("Comfy-Desktop/ComfyUI-Installs");
        let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        let configs = PathBuf::from(roaming).join("Comfy Desktop/instance-model-paths");
        let mut installs: Vec<_> = std::fs::read_dir(&root)
            .map_err(|e| format!("ComfyUI installation not found: {e}. Open Dependencies to install ComfyUI"))?
            .filter_map(Result::ok).map(|e| e.path())
            .filter(|p| p.join("ComfyUI/main.py").is_file()
                && p.join("ComfyUI/.venv/Scripts/python.exe").is_file()).collect();
        installs.sort();
        let install = installs.into_iter().next().ok_or(
            "No ComfyUI Python backend found. Open Dependencies to install ComfyUI, or install ComfyUI Desktop."
        )?;
        let mut command = Command::new(install.join("ComfyUI/.venv/Scripts/python.exe"));
        command.current_dir(&install)
            .arg("-s").arg("ComfyUI/main.py")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .stdin(Stdio::null());
        if let Ok(entries) = std::fs::read_dir(&configs) {
            let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "yaml")).collect();
            files.sort();
            if let Some(config) = files.first() {
                command.arg("--extra-model-paths-config").arg(config);
            }
        }
            command
        };
        command.arg("--disable-auto-launch").arg("--listen").arg("127.0.0.1").arg("--port").arg("8188")
            .stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
        #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        *owned = Some(command.spawn().map_err(|e| format!("Cannot start ComfyUI backend: {e}"))?);
    }
    drop(owned);
    for _ in 0..90 {
        if server_ready() { return Ok(()); }
        std::thread::sleep(Duration::from_secs(1));
        let mut owned = managed().lock().map_err(|e| e.to_string())?;
        if let Some(child) = owned.as_mut() {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                *owned = None;
                return Err(format!("ComfyUI backend exited during startup ({status}). Start it in ComfyUI Desktop to inspect the error."));
            }
        }
    }
    Err("ComfyUI did not become ready within 90 seconds. Check ComfyUI Desktop.".into())
}

pub fn stop_managed_backend() {
    if let Ok(mut guard) = managed().lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}


pub fn checkpoints() -> Result<Vec<String>, String> {
    ensure_server()?;
    let value: Value = client()?.get(format!("{BASE}/object_info/CheckpointLoaderSimple"))
        .send().map_err(|e| format!("Cannot connect to local ComfyUI: {e}"))?
        .error_for_status().map_err(|e| e.to_string())?
        .json().map_err(|e| e.to_string())?;
    let items = value.pointer("/CheckpointLoaderSimple/input/required/ckpt_name/0")
        .and_then(Value::as_array).ok_or("ComfyUI did not return checkpoint names")?;
    Ok(items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

struct ProgressStop(Arc<AtomicBool>);
impl Drop for ProgressStop {
    fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); }
}

pub fn generate(request: Request, cancel: Arc<AtomicBool>, progress: Arc<std::sync::atomic::AtomicU32>) -> Result<image::RgbaImage, String> {
    ensure_server()?;
    if request.prompt.trim().is_empty() { return Err("Enter a generation prompt".into()); }
    if request.checkpoint.trim().is_empty() { return Err("Select a local checkpoint".into()); }
    if !(256..=4096).contains(&request.width) || !(256..=4096).contains(&request.height)
        || request.width % 64 != 0 || request.height % 64 != 0 {
        return Err("Dimensions must be multiples of 64, between 256 and 4096".into());
    }
    if cancel.load(Ordering::Relaxed) { return Err("Generation cancelled".into()); }
    if !(1..=100).contains(&request.steps) {return Err("Steps must be between 1 and 100".into());}
    let qwen = is_qwen(&request.checkpoint);
    let flux=request.checkpoint==FLUX_KLEIN;
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
    if flux {workflow=flux_workflow(None,&request.prompt,request.seed,request.steps,request.width,request.height);}
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
            let resized_mask = image::imageops::resize(&original_mask,request.width,request.height,image::imageops::FilterType::Nearest);
            // Qwen needs the intact reference image; its selection is composited locally.
            if !qwen && !flux {
                for (x,y,pixel) in resized.enumerate_pixels_mut() {
                    pixel[3] = 255u8.saturating_sub(resized_mask.get_pixel(x,y)[0]);
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
    let deadline = Instant::now() + Duration::from_secs(900);
    loop {
        if cancel.load(Ordering::Relaxed) {
            // Cancel only this queued prompt; do not interrupt unrelated ComfyUI jobs.
            let _ = http.post(format!("{BASE}/queue")).json(&json!({"delete":[id]})).send();
            return Err("Generation cancelled".into());
        }
        if Instant::now() >= deadline { return Err("ComfyUI generation timed out after 15 minutes".into()); }
        let history: Value = http.get(format!("{BASE}/history/{id}"))
            .send().map_err(|e| e.to_string())?
            .error_for_status().map_err(|e| e.to_string())?
            .json().map_err(|e| e.to_string())?;
        if let Some(job) = history.get(&id) {
            if job.pointer("/status/status_str").and_then(Value::as_str) == Some("error") {
                return Err(format!("ComfyUI generation failed: {}", job["status"]));
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
                    return finish_edit(output, request.source.as_ref(), request.mask.as_deref());
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
