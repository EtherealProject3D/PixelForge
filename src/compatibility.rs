//! Read-only runtime diagnostics and conservative, mockable compatibility policy.
use serde_json::{Value, json};
use std::{path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};

pub const TORCH_PROBE: &str = r#"
import json, torch
v = {'torch':torch.__version__, 'cuda':torch.version.cuda, 'available':torch.cuda.is_available(), 'devices':[]}
if v['available']:
    v['architectures'] = torch.cuda.get_arch_list()
    for i in range(torch.cuda.device_count()):
        p = torch.cuda.get_device_properties(i)
        v['devices'].append({'name':p.name, 'vram':p.total_memory, 'capability':list(torch.cuda.get_device_capability(i))})
    v['selected'] = torch.cuda.current_device()
    try:
        x = torch.ones((16,16),device='cuda'); y = x @ x; torch.cuda.synchronize()
        v['kernel_test'] = 'passed'
    except Exception as e:
        v['kernel_test'] = str(e)
print('PIXELFORGE_DIAGNOSTIC='+json.dumps(v))
"#;

/// File-backed output avoids pipe deadlock; always bound a child process and reap on timeout.
pub fn capture(mut command: Command, timeout: Duration) -> Result<String, String> {
    let root = crate::profile::settings_root().ok_or("APPDATA unavailable")?.join("diagnostics");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let path = root.join(format!("probe-{}-{id}.log", std::process::id()));
    let output = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    command.stdout(output.try_clone().map_err(|e| e.to_string())?).stderr(output).stdin(Stdio::null());
    #[cfg(windows)] {use std::os::windows::process::CommandExt; command.creation_flags(0x08000000);}
    let result = (|| {
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                _ => {let _ = child.kill(); let _ = child.wait(); return Err("Diagnostic timed out or could not inspect process".into());}
            }
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if status.success() {Ok(text)} else {Err(actionable(&text))}
    })();
    let _ = std::fs::remove_file(path);
    result
}
pub fn python(root: &Path) -> Option<PathBuf> {
    ["python_embeded/python.exe", "ComfyUI/.venv/Scripts/python.exe"]
        .iter().map(|p| root.join(p)).find(|p| p.is_file())
}
pub fn probe(root: &Path) -> Result<Value, String> {
    let mut command = Command::new(python(root).ok_or("Selected ComfyUI folder has no supported Python runtime")?);
    command.current_dir(root).args(["-s", "-c", TORCH_PROBE]);
    let text = capture(command, Duration::from_secs(30))?;
    parse_probe(&text)
}
fn parse_probe(text: &str) -> Result<Value, String> {
    text.lines().find_map(|line| line.strip_prefix("PIXELFORGE_DIAGNOSTIC="))
        .ok_or_else(|| actionable(text)).and_then(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
}
pub fn legacy(capability: (u64,u64)) -> bool { capability.0 <= 6 }
pub fn selected_device(value: &Value) -> Option<&Value> {
    value["devices"].as_array()?.get(value["selected"].as_u64().unwrap_or(0) as usize)
}
pub fn preflight(value: &Value, mode: &str) -> Result<(), String> {
    if !value["available"].as_bool().unwrap_or(false) {
        return Err("CUDA unavailable in the selected Python runtime. Check NVIDIA driver and choose a matching ComfyUI package in Dependencies. No packages were changed.".into());
    }
    let device = selected_device(value).ok_or("No selected CUDA device reported")?;
    let cap = (device["capability"][0].as_u64().ok_or("Missing compute capability")?,device["capability"][1].as_u64().unwrap_or(0));
    if mode == "legacy" && !legacy(cap) {return Err("Legacy backend is for GTX 10-series and older GPUs. Select Auto / Modern for RTX hardware; existing packages were preserved.".into());}
    if mode == "modern" && legacy(cap) {return Err("This GPU needs the legacy CUDA 12.6 ComfyUI package. Choose Legacy in Dependencies. Actual GTX 1080 generation remains unverified.".into());}
    if legacy(cap) && value["cuda"].as_str() != Some("12.6") {
        return Err("GTX 10-series needs a compatible CUDA 12.6 PyTorch build. Register a separate legacy ComfyUI portable folder in Dependencies. Do not upgrade your working environment in place.".into());
    }
    if value["kernel_test"].as_str() != Some("passed") {return Err(actionable(value["kernel_test"].as_str().unwrap_or("CUDA kernel test did not complete")));}
    Ok(())
}
pub fn actionable(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let advice = if lower.contains("out of memory") || lower.contains("outofmemory") {
        "GPU memory exhausted. Lower resolution, use batch 1, close other GPU apps, or choose a smaller model. Check the resolution guidance."
    } else if lower.contains("no kernel image") || lower.contains("invalid device function") || lower.contains("not compatible with the current pytorch") {
        "GPU architecture is unsupported by this PyTorch build. Open Dependencies and run diagnostics. GTX 10-series needs a separate legacy CUDA 12.6 backend; RTX uses the modern backend."
    } else if lower.contains("driver") || lower.contains("cuda initialization") {
        "CUDA driver initialization failed. Update the NVIDIA driver from NVIDIA, restart Windows, and recheck Dependencies. The CUDA version reported by nvidia-smi is the driver limit, not the installed PyTorch runtime."
    } else if lower.contains("fp8") || lower.contains("bfloat16") || lower.contains("not implemented for 'half'") {
        "This model or precision needs newer GPU features. On legacy GPUs try an FP32/FP16 checkpoint with the legacy backend; FP8 models may require modern hardware."
    } else {
        "Open Dependencies > Run GPU diagnostics and inspect the backend log. Check model requirements and the selected ComfyUI environment."
    };
    format!("{advice}\nTechnical detail: {}", raw.chars().take(12000).collect::<String>())
}
pub fn collect() -> Value {
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name,memory.total,memory.free,driver_version,compute_cap", "--format=csv,noheader,nounits"]);
    let hardware = capture(command, Duration::from_secs(10));
    let server = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).build().ok()
        .and_then(|c| c.get("http://127.0.0.1:8188/system_stats").send().ok())
        .and_then(|r| r.error_for_status().ok()).and_then(|r| r.json::<Value>().ok());
    let config = crate::backend::Config::load();
    let selected = config.resolve();
    let runtime = selected.as_ref().map_err(Clone::clone).and_then(|p| probe(p));
    json!({"hardware_csv":hardware.unwrap_or_else(|e|format!("Unavailable (driver tool may not be installed): {e}")),
        "hardware_columns":"name, total VRAM MiB, free VRAM MiB, driver, compute capability",
        "backend_mode":config.mode,"backend_path":selected.ok(),"python_runtime":runtime.unwrap_or_else(|e|json!({"error":e})),
        "running_server":server,"note":"Running server may be externally managed and use a different runtime. GTX 1080 generation is unverified pending hardware test. Diagnostics are read-only; the small CUDA kernel test is not a generation test."})
}
pub fn vram_gib(report: &Value) -> Option<f64> {
    // Prefer the running server's device over an environment that may not be serving requests.
    let server = report["running_server"]["devices"].as_array().and_then(|d| d.first());
    server.and_then(|d|d["vram_total"].as_u64()).or_else(||selected_device(&report["python_runtime"]).and_then(|d|d["vram"].as_u64()))
        .map(|n| n as f64 / 1_073_741_824.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn runtime(cap:u64,cuda:&str,kernel:&str)->Value {json!({"available":true,"cuda":cuda,"selected":0,"kernel_test":kernel,"devices":[{"capability":[cap,1],"vram":8589934592u64}]})}
    #[test] fn gtx1080_requires_legacy_and_real_kernel_probe() {
        assert!(preflight(&runtime(6,"12.6","passed"),"legacy").is_ok());
        assert!(preflight(&runtime(6,"13.0","passed"),"auto").is_err());
        assert!(preflight(&runtime(6,"12.6","no kernel image is available"),"auto").unwrap_err().contains("architecture"));
        assert!(preflight(&runtime(6,"12.6","passed"),"modern").is_err());
    }
    #[test] fn blackwell_never_routes_to_legacy() {
        assert!(preflight(&runtime(12,"13.0","passed"),"modern").is_ok());
        assert!(preflight(&runtime(12,"13.0","passed"),"legacy").is_err());
    }
    #[test] fn error_advice_preserves_details_and_unknowns() {
        assert!(actionable("CUDA out of memory: 123").contains("Lower resolution"));
        assert!(actionable("invalid device function").contains("12.6"));
        assert!(actionable("Mystery failure 42").contains("Mystery failure 42"));
        assert!(parse_probe("warning\nPIXELFORGE_DIAGNOSTIC={\"available\":false}").is_ok());
    }
}
