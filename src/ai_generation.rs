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
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
        let root = PathBuf::from(local).join("Comfy-Desktop/ComfyUI-Installs");
        let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        let configs = PathBuf::from(roaming).join("Comfy Desktop/instance-model-paths");
        let mut installs: Vec<_> = std::fs::read_dir(&root)
            .map_err(|e| format!("ComfyUI Desktop installation not found: {e}"))?
            .filter_map(Result::ok).map(|e| e.path())
            .filter(|p| p.join("ComfyUI/main.py").is_file()
                && p.join("ComfyUI/.venv/Scripts/python.exe").is_file()).collect();
        installs.sort();
        let install = installs.into_iter().next().ok_or(
            "No ComfyUI Desktop Python backend found. Install ComfyUI Desktop first."
        )?;
        let mut command = Command::new(install.join("ComfyUI/.venv/Scripts/python.exe"));
        command.current_dir(&install)
            .arg("-s").arg("ComfyUI/main.py")
            .arg("--listen").arg("127.0.0.1").arg("--port").arg("8188")
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
    if !(256..=2048).contains(&request.width) || !(256..=2048).contains(&request.height)
        || request.width % 64 != 0 || request.height % 64 != 0 {
        return Err("Dimensions must be multiples of 64, between 256 and 2048".into());
    }
    if cancel.load(Ordering::Relaxed) { return Err("Generation cancelled".into()); }
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
            for (x,y,pixel) in resized.enumerate_pixels_mut() {
                pixel[3] = 255u8.saturating_sub(resized_mask.get_pixel(x,y)[0]);
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
                    return image::load_from_memory(&bytes).map(|img| img.to_rgba8())
                        .map_err(|e| format!("Cannot decode generated image: {e}"));
                }
            }
            if job.get("outputs").is_some() { return Err("ComfyUI finished without a saved image".into()); }
        }
        std::thread::sleep(Duration::from_millis(750));
    }
}
