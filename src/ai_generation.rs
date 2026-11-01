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
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(25))
        .build().map_err(|e| e.to_string())
}

const BASE: &str = "http://127.0.0.1:8188";

pub fn checkpoints() -> Result<Vec<String>, String> {
    let value: Value = client()?.get(format!("{BASE}/object_info/CheckpointLoaderSimple"))
        .send().map_err(|e| format!("Cannot connect to local ComfyUI: {e}"))?
        .error_for_status().map_err(|e| e.to_string())?
        .json().map_err(|e| e.to_string())?;
    let items = value.pointer("/CheckpointLoaderSimple/input/required/ckpt_name/0")
        .and_then(Value::as_array).ok_or("ComfyUI did not return checkpoint names")?;
    Ok(items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

pub fn generate(request: Request, cancel: Arc<AtomicBool>) -> Result<image::RgbaImage, String> {
    if request.prompt.trim().is_empty() { return Err("Enter a generation prompt".into()); }
    if request.checkpoint.trim().is_empty() { return Err("Select a local checkpoint".into()); }
    if !(256..=2048).contains(&request.width) || !(256..=2048).contains(&request.height)
        || request.width % 64 != 0 || request.height % 64 != 0 {
        return Err("Dimensions must be multiples of 64, between 256 and 2048".into());
    }
    if cancel.load(Ordering::Relaxed) { return Err("Generation cancelled".into()); }
    let workflow = json!({
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
    let response: Value = http.post(format!("{BASE}/prompt")).json(&json!({"prompt":workflow}))
        .send().map_err(|e| format!("Cannot submit ComfyUI prompt: {e}"))?
        .error_for_status().map_err(|e| format!("ComfyUI rejected workflow: {e}"))?
        .json().map_err(|e| e.to_string())?;
    let id = response.get("prompt_id").and_then(Value::as_str)
        .ok_or_else(|| format!("ComfyUI did not accept the prompt: {response}"))?.to_owned();
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
