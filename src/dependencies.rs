//! Dependency inventory and side-by-side installation with reversible activation.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Comfy,
    Denoise,
    Upscale,
    DeblurModel,
    DeblurRuntime,
}
impl Component {
    pub fn id(self) -> &'static str {
        match self {
            Self::Comfy => "comfyui",
            Self::Denoise => "waifu2x",
            Self::Upscale => "realesrgan",
            Self::DeblurModel => "nafnet",
            Self::DeblurRuntime => "deblur-python",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Comfy => "ComfyUI generation backend",
            Self::Denoise => "Waifu2x denoise engine and models",
            Self::Upscale => "Real-ESRGAN upscale / sharpen",
            Self::DeblurModel => "NAFNet deblur model",
            Self::DeblurRuntime => "Deblur Python runtime and packages",
        }
    }
    fn marker(self) -> &'static str {
        match self {
            Self::Comfy => "ComfyUI/main.py",
            Self::Denoise => "waifu2x-ncnn-vulkan.exe",
            Self::Upscale => "realesrgan-ncnn-vulkan.exe",
            Self::DeblurModel => "deblurring_nafnet_2025may.onnx",
            Self::DeblurRuntime => "python.exe",
        }
    }
}
const COMPONENTS: [Component; 5] = [
    Component::Comfy,
    Component::Denoise,
    Component::Upscale,
    Component::DeblurModel,
    Component::DeblurRuntime,
];
#[cfg(test)]
thread_local! {static TEST_ROOT:std::cell::RefCell<Option<PathBuf>>=const {std::cell::RefCell::new(None)};}
pub fn root() -> PathBuf {
    #[cfg(test)]
    if let Some(root) = TEST_ROOT.with(|r| r.borrow().clone()) {
        return root;
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools"))
        .join("PixelForge/deps")
}
fn component_root(c: Component) -> PathBuf {
    root().join(c.id())
}
fn registry(c: Component) -> Value {
    fs::read(component_root(c).join("active.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}))
}
fn registry_path(value: &Value, key: &str) -> Option<PathBuf> {
    value[key].as_str().map(PathBuf::from)
}
fn inside(root: &Path, path: &Path) -> bool {
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(path)) => path.starts_with(&root) && path != root,
        _ => false,
    }
}
fn complete(c: Component, path: &Path) -> bool {
    if !path.join(c.marker()).is_file() {
        return false;
    }
    match c {
        Component::Denoise => (0..=3).all(|n| {
            ["bin", "param"].iter().all(|ext| {
                path.join(format!("models-cunet/noise{n}_model.{ext}"))
                    .is_file()
            })
        }),
        Component::Upscale => ["realesrgan-x4plus", "realesrgan-x4plus-anime"]
            .iter()
            .all(|name| {
                ["bin", "param"]
                    .iter()
                    .all(|ext| path.join(format!("models/{name}.{ext}")).is_file())
            }),
        Component::Comfy => {
            path.join("python_embeded/python.exe").is_file()
                || path.join("ComfyUI/.venv/Scripts/python.exe").is_file()
        }
        Component::DeblurModel => {
            fs::metadata(path.join(c.marker())).is_ok_and(|m| m.len() == 91_736_251)
        }
        Component::DeblurRuntime => true,
    }
}
pub fn active_path(c: Component) -> Option<PathBuf> {
    let path = registry_path(&registry(c), "active")?;
    (inside(&component_root(c).join("versions"), &path) && complete(c, &path)).then_some(path)
}
pub fn deblur_model_path() -> Option<PathBuf> {
    let folder =
        active_path(Component::DeblurModel).or_else(|| original(Component::DeblurModel))?;
    complete(Component::DeblurModel, &folder).then(|| folder.join(Component::DeblurModel.marker()))
}
pub fn deblur_python() -> Option<PathBuf> {
    active_path(Component::DeblurRuntime).map(|p| p.join("python.exe"))
}
fn original(c: Component) -> Option<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(folder) = std::env::current_exe().ok().and_then(|exe| {
        exe.parent().map(|p| {
            p.join("tools").join(match c {
                Component::Denoise => "waifu2x",
                Component::Upscale => "realesrgan",
                Component::DeblurModel => "restoration",
                _ => "unused",
            })
        })
    }) {
        if folder.join(c.marker()).is_file() {
            return Some(folder);
        }
    }
    match c {
        Component::Denoise => Some(base.join("tools/waifu2x")),
        Component::Upscale => Some(base.join("tools/realesrgan")),
        Component::DeblurModel => Some(base.join("tools/restoration")),
        Component::Comfy => desktop_backend(),
        Component::DeblurRuntime => None,
    }
}
pub fn desktop_backend() -> Option<PathBuf> {
    let root =
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Comfy-Desktop/ComfyUI-Installs");
    let mut paths: Vec<_> = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.join("ComfyUI/main.py").is_file()
                && p.join("ComfyUI/.venv/Scripts/python.exe").is_file()
        })
        .collect();
    paths.sort();
    paths.into_iter().next()
}
fn write_registry(c: Component, value: &Value) -> Result<(), String> {
    let folder = component_root(c);
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let temporary = folder.join("active.json.new");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, folder.join("active.json")).map_err(|e| e.to_string())
}
fn activate(c: Component, path: &Path, version: &str) -> Result<(), String> {
    if !inside(&component_root(c).join("versions"), path) || !complete(c, path) {
        return Err("Incomplete dependency installation; current version was kept".into());
    }
    let old = registry(c);
    let previous = active_path(c);
    let previous_version = if previous.is_some() {
        Some(
            old["version"]
                .as_str()
                .unwrap_or("Previous version")
                .to_owned(),
        )
    } else if original(c).is_some_and(|p| complete(c, &p)) {
        Some("Original installation".into())
    } else if c == Component::DeblurRuntime && system_python_ready() {
        Some("System Python installation".into())
    } else {
        None
    };
    write_registry(
        c,
        &json!({"active":path,"version":version,"previous":previous,"previous_version":previous_version}),
    )
}
pub fn rollback(c: Component) -> Result<String, String> {
    let value = registry(c);
    let version = value["previous_version"]
        .as_str()
        .ok_or("No previous version is saved")?;
    let previous = registry_path(&value, "previous");
    if let Some(path) = &previous {
        if !inside(&component_root(c).join("versions"), path) || !complete(c, path) {
            return Err("The saved previous installation is unavailable".into());
        }
    } else if c == Component::DeblurRuntime {
        if !system_python_ready() {
            return Err("Previous system Python dependencies are unavailable".into());
        }
    } else if !original(c).is_some_and(|p| complete(c, &p)) {
        return Err("Original installation is unavailable".into());
    }
    write_registry(
        c,
        &json!({"active":previous,"version":version,"previous":value["active"],"previous_version":value["version"]}),
    )?;
    Ok(format!(
        "Restored {}. Restart any running backend to use it.",
        c.title()
    ))
}
fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}
fn python_command() -> Command {
    if let Some(python) = deblur_python() {
        Command::new(python)
    } else {
        let mut command = Command::new("py");
        command.arg("-3");
        command
    }
}
fn python_probe(mut command: Command) -> Result<String, String> {
    hidden(&mut command);
    let output=command.args(["-c","import sys,cv2,numpy,onnxruntime; print('Python '+sys.version.split()[0]+' | OpenCV '+cv2.__version__+' | NumPy '+numpy.__version__+' | ONNX Runtime '+onnxruntime.__version__)"]).stdin(Stdio::null()).output().map_err(|e|e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(800)
            .collect())
    }
}
fn python_ready(command: Command) -> bool {
    python_probe(command).is_ok()
}
fn system_python_ready() -> bool {
    let mut command = Command::new("py");
    command.arg("-3");
    python_ready(command)
}
#[derive(Clone)]
pub struct Status {
    pub component: Option<Component>,
    pub title: String,
    pub required: bool,
    pub installed: bool,
    pub detail: String,
    pub rollback: bool,
}
impl Status {
    fn category(&self) -> usize {
        match self.component {
            Some(Component::Comfy) => 0,
            Some(Component::Denoise | Component::DeblurModel | Component::DeblurRuntime) => 1,
            Some(Component::Upscale) => 2,
            None if self.title.to_ascii_lowercase().contains("model") => 0,
            None => 3,
        }
    }
}
pub fn scan(models: Option<PathBuf>, known: Vec<String>) -> Vec<Status> {
    let models = models.or_else(|| crate::model_installer::detect_models_folder(&known));
    let mut rows = Vec::new();
    for c in COMPONENTS {
        let folder = active_path(c).or_else(|| original(c));
        let mut installed = folder.as_ref().is_some_and(|p| complete(c, p));
        if c == Component::DeblurModel {
            installed &= folder.as_ref().is_some_and(|p| {
                fs::metadata(p.join(c.marker())).is_ok_and(|m| m.len() == 91_736_251)
            });
        }
        if c == Component::Denoise {
            installed &= folder.as_ref().is_some_and(|p| {
                (0..=3).all(|n| {
                    ["bin", "param"].iter().all(|ext| {
                        p.join(format!("models-cunet/noise{n}_model.{ext}"))
                            .is_file()
                    })
                })
            });
        }
        if c == Component::Upscale {
            installed &= folder.as_ref().is_some_and(|p| {
                ["realesrgan-x4plus", "realesrgan-x4plus-anime"]
                    .iter()
                    .all(|name| {
                        ["bin", "param"]
                            .iter()
                            .all(|ext| p.join(format!("models/{name}.{ext}")).is_file())
                    })
            });
        }
        let python = if c == Component::DeblurRuntime {
            Some(python_probe(python_command()))
        } else {
            None
        };
        if let Some(result) = &python {
            installed = result.is_ok();
        }
        let state = registry(c);
        let version = state["version"].as_str().unwrap_or("Existing installation");
        rows.push(Status {
            component: Some(c),
            title: c.title().into(),
            required: true,
            installed,
            detail: if let Some(result) = python {
                result.unwrap_or_else(|e| format!("Python / packages need setup: {e}"))
            } else if installed {
                version.into()
            } else {
                "Missing or incomplete".into()
            },
            rollback: state["previous_version"].is_string(),
        });
    }
    let vulkan = crate::ai_upscale::available_vulkan_gpus();
    rows.push(Status {
        component: None,
        title: "Vulkan GPU driver".into(),
        required: true,
        installed: vulkan.as_ref().is_ok_and(|g| !g.is_empty()),
        detail: vulkan
            .map(|g| {
                g.iter()
                    .map(|g| g.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|e| e),
        rollback: false,
    });
    let generation = !known.is_empty()
        || models.as_ref().is_some_and(|root| {
            crate::model_installer::MODELS
                .iter()
                .filter(|m| m.generation)
                .any(|m| crate::model_installer::installed_files(m, root) == m.files.len())
        });
    rows.push(Status {
        component: None,
        title: "Image generation model".into(),
        required: true,
        installed: generation,
        detail: "Install or remove checkpoints in Model Library".into(),
        rollback: false,
    });
    let qwen = models.as_ref().is_some_and(|root| {
        crate::model_installer::MODELS
            .iter()
            .filter(|m| crate::ai_generation::is_qwen(m.name))
            .any(|m| crate::model_installer::installed_files(m, root) == m.files.len())
    });
    rows.push(Status {
        component: None,
        title: "Qwen instruction-edit model (optional)".into(),
        required: false,
        installed: qwen,
        detail: "Three-file bundle in Model Library".into(),
        rollback: false,
    });
    rows
}

#[derive(Clone)]
pub struct Transfer {
    pub bytes: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
    pub message: Arc<Mutex<String>>,
}
impl Default for Transfer {
    fn default() -> Self {
        Self {
            bytes: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            message: Arc::new(Mutex::new(String::new())),
        }
    }
}
impl Transfer {
    fn say(&self, message: impl Into<String>) {
        if let Ok(mut value) = self.message.lock() {
            *value = message.into();
        }
    }
    fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("Cancelled; current dependency remains active".into())
        } else {
            Ok(())
        }
    }
}
fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("PixelForge dependency manager")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(6 * 3600))
        .build()
        .map_err(|e| e.to_string())
}
fn download(url: &str, path: &Path, max: u64, transfer: &Transfer) -> Result<(), String> {
    transfer.check()?;
    transfer.say(format!(
        "Downloading {}",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let mut response = http()?
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let length = response
        .content_length()
        .ok_or("Download size unavailable")?;
    if length == 0 || length > max {
        return Err(format!("Unexpected dependency download size: {length}"));
    }
    let free = fs2::available_space(path.parent().ok_or("Invalid download path")?)
        .map_err(|e| e.to_string())?;
    if free < length + 512_000_000 {
        return Err("Not enough disk space for dependency download".into());
    }
    transfer.total.store(length, Ordering::Relaxed);
    transfer.bytes.store(0, Ordering::Relaxed);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 256 * 1024];
    let mut count = 0;
    loop {
        transfer.check()?;
        let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > length {
            return Err("Download larger than expected".into());
        }
        output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        transfer.bytes.store(count, Ordering::Relaxed);
    }
    output.sync_all().map_err(|e| e.to_string())?;
    if count != length {
        return Err("Incomplete dependency download".into());
    }
    Ok(())
}
fn run(command: &mut Command, log: &Path, transfer: &Transfer) -> Result<(), String> {
    transfer.check()?;
    hidden(command);
    let output = fs::File::create(log).map_err(|e| e.to_string())?;
    command
        .stdout(output.try_clone().map_err(|e| e.to_string())?)
        .stderr(output)
        .stdin(Stdio::null());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    loop {
        if transfer.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return transfer.check();
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                let bytes = fs::read(log).unwrap_or_default();
                let tail = &bytes[bytes.len().saturating_sub(2000)..];
                return Err(format!(
                    "Setup exited {status}: {}",
                    String::from_utf8_lossy(tail)
                ));
            }
            None => std::thread::sleep(Duration::from_millis(150)),
        }
    }
}
fn expand_zip(
    archive: &Path,
    destination: &Path,
    log: &Path,
    transfer: &Transfer,
) -> Result<(), String> {
    transfer.say("Extracting dependency files…");
    run(Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; [System.Reflection.Assembly]::LoadWithPartialName('System.IO.Compression.FileSystem') > $null; [System.IO.Compression.ZipFile]::ExtractToDirectory($env:PF_DEP_ARCHIVE,$env:PF_DEP_DEST)"]).env("PF_DEP_ARCHIVE",archive).env("PF_DEP_DEST",destination),log,transfer)
}
fn find_folder(root: &Path, marker: &str, depth: usize) -> Option<PathBuf> {
    if root.join(marker).is_file() {
        return Some(root.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .find_map(|e| find_folder(&e.path(), marker, depth - 1))
}
fn release(
    repo: &str,
    asset_filter: impl Fn(&str) -> bool,
) -> Result<(String, String, Option<String>), String> {
    let value: Value = http()?
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let version = value["tag_name"]
        .as_str()
        .ok_or("Release version unavailable")?
        .to_owned();
    let asset = value["assets"]
        .as_array()
        .ok_or("Release assets unavailable")?
        .iter()
        .find(|a| a["name"].as_str().is_some_and(&asset_filter))
        .ok_or("Official Windows release asset unavailable")?;
    Ok((
        version,
        asset["browser_download_url"]
            .as_str()
            .ok_or("Download URL unavailable")?
            .to_owned(),
        asset["digest"].as_str().map(str::to_owned),
    ))
}
fn verify_hash(path: &Path, expected: &str, log: &Path, transfer: &Transfer) -> Result<(), String> {
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
    run(Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; $stream=[System.IO.File]::OpenRead($env:PF_DEP_ARCHIVE); try {$algorithm=[System.Security.Cryptography.SHA256]::Create(); $hash=[System.BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-',''); if ($hash -ne $env:PF_DEP_HASH) {throw 'Dependency SHA256 mismatch'}} finally {$stream.Dispose()}"]).env("PF_DEP_ARCHIVE",path).env("PF_DEP_HASH",expected),log,transfer)
}
fn safe_cleanup(c: Component, path: &Path) {
    if inside(&component_root(c).join("versions"), path) {
        let _ = fs::remove_dir_all(path);
    }
}

pub fn install(c: Component, gpu: &str, transfer: &Transfer) -> Result<String, String> {
    transfer.check()?;
    if c == Component::Comfy && !["nvidia", "nvidia_cu126", "amd", "intel"].contains(&gpu) {
        return Err("Unsupported ComfyUI hardware package".into());
    }
    let versions = component_root(c).join("versions");
    fs::create_dir_all(&versions).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let stage = versions.join(format!("v{:x}", stamp / 1_000_000));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result=install_stage(c,gpu,&stage,transfer).and_then(|(payload,version)| {
        transfer.check()?;
        activate(c,&payload,&version)?;
        Ok(format!("{} ready ({version}). Previous installation kept for rollback. Restart any running backend.",c.title()))
    });
    if result.is_err() {
        safe_cleanup(c, &stage);
    }
    result
}
fn install_stage(
    c: Component,
    gpu: &str,
    stage: &Path,
    transfer: &Transfer,
) -> Result<(PathBuf, String), String> {
    let log = stage.join("setup.log");
    if c == Component::DeblurModel {
        if registry(c)["version"] == "OpenCV NAFNet 2025may" && active_path(c).is_some() {
            return Err("NAFNet is already at the supported release; no files were changed".into());
        }
        let output = stage.join(c.marker());
        download(
            "https://huggingface.co/opencv/deblurring_nafnet/resolve/main/deblurring_nafnet_2025may.onnx",
            &output,
            200_000_000,
            transfer,
        )?;
        verify_hash(
            &output,
            "07263f416febecce10193dd648e950b22e397cf521eedab1a114ef77b2bc9587",
            &log,
            transfer,
        )?;
        return Ok((stage.to_path_buf(), "OpenCV NAFNet 2025may".into()));
    }
    if c == Component::DeblurRuntime {
        let archive = stage.join("python.zip");
        download(
            "https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip",
            &archive,
            40_000_000,
            transfer,
        )?;
        verify_hash(
            &archive,
            "4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3",
            &log,
            transfer,
        )?;
        let payload = stage.join("runtime");
        expand_zip(&archive, &payload, &log, transfer)?;
        fs::write(
            payload.join("python312._pth"),
            "python312.zip\n.\nLib/site-packages\nimport site\n",
        )
        .map_err(|e| e.to_string())?;
        let bootstrap = stage.join("get-pip.py");
        download(
            "https://bootstrap.pypa.io/get-pip.py",
            &bootstrap,
            5_000_000,
            transfer,
        )?;
        let python = payload.join("python.exe");
        transfer.say("Installing isolated deblur packages…");
        run(
            Command::new(&python)
                .arg(&bootstrap)
                .args(["--no-warn-script-location", "--disable-pip-version-check"]),
            &log,
            transfer,
        )?;
        run(
            Command::new(&python).args([
                "-m",
                "pip",
                "install",
                "--upgrade",
                "--only-binary=:all:",
                "--disable-pip-version-check",
                "onnxruntime",
                "opencv-python-headless",
                "numpy",
            ]),
            &log,
            transfer,
        )?;
        run(Command::new(&python).args(["-c","import cv2,numpy,onnxruntime; assert 'CPUExecutionProvider' in onnxruntime.get_available_providers()"]),&log,transfer)?;
        let _ = fs::remove_file(archive);
        let _ = fs::remove_file(bootstrap);
        return Ok((payload, "Python 3.12.10 + current deblur packages".into()));
    }
    let (mut version, url, digest) = match c {
        Component::Comfy => release("Comfy-Org/ComfyUI", |name| {
            name == format!("ComfyUI_windows_portable_{gpu}.7z")
        })?,
        Component::Denoise => release("nihui/waifu2x-ncnn-vulkan", |name| {
            name.ends_with("-windows.zip")
        })?,
        Component::Upscale => release("xinntao/Real-ESRGAN-ncnn-vulkan", |name| {
            name.ends_with("-windows.zip")
        })?,
        _ => unreachable!(),
    };
    if c == Component::Comfy {
        version = format!("{version} ({gpu})");
    }
    if registry(c)["version"].as_str() == Some(&version) && active_path(c).is_some() {
        return Err(format!(
            "{} is already at the latest published release ({version}). No changes were made.",
            c.title()
        ));
    }
    if c == Component::Comfy {
        let free = fs2::available_space(stage).map_err(|e| e.to_string())?;
        if free < 15_000_000_000 {
            return Err("Allow at least 15 GB free for a new ComfyUI portable installation, in addition to retained versions".into());
        }
    }
    let archive = stage.join(if c == Component::Comfy {
        "download.7z"
    } else {
        "download.zip"
    });
    download(&url, &archive, 4_000_000_000, transfer)?;
    if let Some(digest) = digest {
        verify_hash(&archive, &digest, &log, transfer)?;
    }
    let payload = stage.join("payload");
    if c == Component::Comfy {
        let extractor = stage.join("7zr.exe");
        download(
            "https://www.7-zip.org/a/7zr.exe",
            &extractor,
            3_000_000,
            transfer,
        )?;
        transfer.say("Extracting ComfyUI and its embedded Python/CUDA environment…");
        run(
            Command::new(&extractor)
                .arg("x")
                .arg(&archive)
                .arg(format!("-o{}", payload.display()))
                .arg("-y"),
            &log,
            transfer,
        )?;
        let folder = find_folder(&payload, c.marker(), 3)
            .ok_or("ComfyUI archive layout is not recognized")?;
        if !folder.join("python_embeded/python.exe").is_file() {
            return Err("ComfyUI embedded Python missing".into());
        }
        let health = if gpu.starts_with("nvidia") {
            "import torch,safetensors; assert torch.cuda.is_available(), 'NVIDIA CUDA unavailable: update the GPU driver or choose a compatible package'"
        } else if gpu == "intel" {
            "import torch,safetensors; assert torch.xpu.is_available(), 'Intel GPU unavailable: check the GPU driver'"
        } else {
            "import torch,safetensors"
        };
        run(
            Command::new(folder.join("python_embeded/python.exe")).args(["-s", "-c", health]),
            &log,
            transfer,
        )?;
        let _ = fs::remove_file(archive);
        let _ = fs::remove_file(extractor);
        return Ok((folder, version));
    }
    expand_zip(&archive, &payload, &log, transfer)?;
    let folder =
        find_folder(&payload, c.marker(), 3).ok_or("Engine archive layout is not recognized")?;
    if c == Component::Upscale {
        // The official NCNN executable release carries only the anime-video weights.
        // Obtain the photo/anime weights from the official full Windows package.
        let complete = ["realesrgan-x4plus", "realesrgan-x4plus-anime"]
            .iter()
            .all(|name| {
                ["bin", "param"]
                    .iter()
                    .all(|ext| folder.join(format!("models/{name}.{ext}")).is_file())
            });
        if !complete {
            let weights = stage.join("weights.zip");
            download(
                "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-windows.zip",
                &weights,
                100_000_000,
                transfer,
            )?;
            let unpacked = stage.join("weights");
            expand_zip(&weights, &unpacked, &log, transfer)?;
            let model_folder = find_folder(&unpacked, "models/realesrgan-x4plus.bin", 3)
                .ok_or("Official Real-ESRGAN weights not found")?;
            fs::create_dir_all(folder.join("models")).map_err(|e| e.to_string())?;
            for name in ["realesrgan-x4plus", "realesrgan-x4plus-anime"] {
                for ext in ["bin", "param"] {
                    fs::copy(
                        model_folder.join(format!("models/{name}.{ext}")),
                        folder.join(format!("models/{name}.{ext}")),
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            let _ = fs::remove_file(weights);
            fs::remove_dir_all(unpacked).map_err(|e| e.to_string())?;
        }
    }
    if c == Component::Denoise
        && !(0..=3).all(|n| {
            ["bin", "param"].iter().all(|ext| {
                folder
                    .join(format!("models-cunet/noise{n}_model.{ext}"))
                    .is_file()
            })
        })
    {
        return Err("Denoise archive is missing required models".into());
    }
    run(
        Command::new(folder.join(c.marker())).arg("-h"),
        &log,
        transfer,
    )
    .or_else(|e| {
        // NCNN engines return nonzero after displaying their usage help.
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains("Usage:") || text.contains("Usage ") {
            Ok(())
        } else {
            Err(e)
        }
    })?;
    let _ = fs::remove_file(archive);
    Ok((folder, version))
}

pub struct Manager {
    pub open: bool,
    pub rows: Vec<Status>,
    pub message: String,
    pub transfer: Transfer,
    pub gpu: String,
    rx: Option<std::sync::mpsc::Receiver<Result<Vec<Status>, String>>>,
    scanned: bool,
    inventory: Option<(Option<PathBuf>, Vec<String>)>,
    active_component: Option<Component>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            open: false,
            rows: Vec::new(),
            message: String::new(),
            transfer: Transfer::default(),
            gpu: "nvidia".into(),
            rx: None,
            scanned: false,
            inventory: None,
            active_component: None,
        }
    }
}
impl Manager {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn invalidate(&mut self) {
        self.scanned = false;
    }
    pub fn missing(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| !r.installed && r.required)
            .count()
    }
    pub fn refresh(&mut self, models: Option<PathBuf>, known: Vec<String>) {
        if self.busy() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.active_component = None;
        self.message = "Checking dependencies…".into();
        std::thread::spawn(move || {
            let _ = tx.send(Ok(scan(models, known)));
        });
    }
    pub fn poll(&mut self, ctx: &egui::Context, models: Option<PathBuf>, known: Vec<String>) {
        if !self.busy()
            && (!self.scanned || self.inventory.as_ref() != Some(&(models.clone(), known.clone())))
        {
            self.scanned = true;
            self.inventory = Some((models.clone(), known.clone()));
            self.refresh(models, known);
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.rx = None;
                    match result {
                        Ok(rows) => {
                            self.rows = rows;
                            if self.missing() > 0 && !root().join("welcome-seen").exists() {
                                self.open = true;
                                let _ = fs::create_dir_all(root());
                                let _ = fs::write(root().join("welcome-seen"), b"seen");
                            }
                            if self.message == "Checking dependencies…" {
                                self.message = "Dependency check complete".into();
                            } else {
                                self.message = self
                                    .transfer
                                    .message
                                    .lock()
                                    .map(|m| m.clone())
                                    .unwrap_or_default();
                            }
                        }
                        Err(e) => self.message = e,
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.message = "Dependency worker ended unexpectedly".into();
                }
                Err(_) => ctx.request_repaint_after(Duration::from_millis(200)),
            }
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        models: Option<PathBuf>,
        known: Vec<String>,
        workspace_busy: bool,
    ) -> bool {
        if !self.open {
            return false;
        }
        let mut open = self.open;
        let mut library = false;
        let mut action = None;
        let mut refresh = false;
        egui::Window::new("Dependencies").open(&mut open).default_width(650.0).default_height(740.0).default_pos(ctx.screen_rect().center()-egui::vec2(325.0,370.0)).resizable(true).vscroll(true).show(ctx,|ui| {
            ui.heading("PixelForge setup and maintenance");
            ui.label("Install missing components here. Updates use separate folders and keep the previous installation for rollback. Your existing ComfyUI Desktop setup is left untouched.");
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.busy(),egui::Button::new("Refresh Status")).clicked() {refresh=true;}
                if ui.button("Open Model Library").clicked() {library=true;}
            });
            ui.label("ComfyUI portable hardware:");
            let hardware=[("nvidia","NVIDIA RTX (recommended)"),("nvidia_cu126","Older NVIDIA GPUs"),("amd","AMD"),("intel","Intel")];
            let label=hardware.iter().find(|(id,_)|*id==self.gpu).map(|(_,label)|*label).unwrap_or("Choose GPU");
            egui::ComboBox::from_id_salt("dependency_hardware").selected_text(label).show_ui(ui,|ui| {for (id,label) in hardware {ui.selectable_value(&mut self.gpu,id.into(),label);}});
            for (category, (heading, description)) in [
                ("Generation & AI Restore", "ComfyUI and models for image generation and instruction-based editing."),
                ("Photo Cleanup", "Noise reduction and deblurring."),
                ("Upscaling & Sharpening", "Image enlargement and detail enhancement."),
                ("GPU Support", "Graphics driver support for local AI tools."),
            ].into_iter().enumerate() {
                if !self.rows.iter().any(|row|row.category()==category) {continue;}
                ui.add_space(8.0);
                crate::theme::section(ui,heading);
                ui.label(description);
            for row in self.rows.iter().filter(|row|row.category()==category) {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(&row.title);
                    ui.colored_label(if row.installed {egui::Color32::LIGHT_GREEN} else {egui::Color32::YELLOW},if row.installed {"Installed"} else {"Missing / needs setup"});
                    ui.label(&row.detail);
                    if let Some(c)=row.component {
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(!self.busy()&&!workspace_busy,egui::Button::new(if row.installed {"Check / Install Update"} else {"Install"})).clicked() {action=Some((c,false));}
                            if ui.add_enabled(row.rollback&&!self.busy()&&!workspace_busy,egui::Button::new("Roll Back")).clicked() {action=Some((c,true));}
                        });
                        if self.active_component==Some(c) {
                            if self.busy() {
                                let total=self.transfer.total.load(Ordering::Relaxed);
                                let bytes=self.transfer.bytes.load(Ordering::Relaxed);
                                let label=if total>0 {format!("{:.1} / {:.1} MB",bytes as f64/1e6,total as f64/1e6)} else {"Preparing / installing…".into()};
                                ui.add(egui::ProgressBar::new(if total>0 {(bytes as f32/total as f32).clamp(0.0,1.0)} else {0.0}).animate(true).text(label));
                                ui.label(self.transfer.message.lock().map(|v|v.clone()).unwrap_or_default());
                                if ui.button("Cancel Setup").clicked() {self.transfer.cancel.store(true,Ordering::Relaxed);}
                            } else {ui.label(&self.message);}
                        }
                    } else if row.title.contains("model") {
                        if ui.button("Manage Models").clicked() {library=true;}
                    } else {
                        ui.hyperlink_to("GPU driver setup","https://docs.comfy.org/installation/system_requirements");
                    }
                });
            }
            }
            if workspace_busy {ui.label("Finish generation, image processing, or model downloads before changing dependencies.");}
            if self.active_component.is_none() {
                if self.busy() {ui.spinner();}
                ui.label(&self.message);
            }
            ui.label(format!("Managed installation folder: {}",root().display()));
            ui.label("Retained versions use additional disk space. Keep old versions until you have tested the update. Portable ComfyUI includes Python and CUDA; the separate deblur runtime includes Python, ONNX Runtime, NumPy and OpenCV. GPU drivers are managed by the GPU vendor.");
        });
        self.open = open && !library;
        if refresh {
            self.refresh(models.clone(), known.clone());
        }
        if let Some((component, rollback_requested)) = action {
            self.active_component = Some(component);
            let (tx, rx) = std::sync::mpsc::channel();
            self.rx = Some(rx);
            self.transfer = Transfer::default();
            let transfer = self.transfer.clone();
            let gpu = self.gpu.clone();
            self.message = format!("Setting up {}…", component.title());
            std::thread::spawn(move || {
                if component == Component::Comfy {
                    crate::ai_generation::stop_managed_backend();
                }
                let result = if rollback_requested {
                    rollback(component)
                } else {
                    install(component, &gpu, &transfer)
                };
                transfer.say(
                    result
                        .as_ref()
                        .map(|m| m.clone())
                        .unwrap_or_else(|e| e.clone()),
                );
                let _ = tx.send(result.map(|_| scan(models, known)));
            });
        }
        library
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_progress_is_between_active_component_and_next_component() {
        let ctx=egui::Context::default();
        let mut manager=Manager::default();
        manager.open=true;
        manager.active_component=Some(Component::Denoise);
        let (_tx,rx)=std::sync::mpsc::channel();manager.rx=Some(rx);
        manager.transfer.bytes.store(1_000_000,Ordering::Relaxed);
        manager.transfer.total.store(2_000_000,Ordering::Relaxed);
        manager.rows=vec![
            Status{component:Some(Component::Denoise),title:"Active component".into(),required:true,installed:false,detail:"Test".into(),rollback:false},
            Status{component:Some(Component::Upscale),title:"Next component".into(),required:true,installed:false,detail:"Test".into(),rollback:false}];
        let input=||egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.0,1200.0))),..Default::default()};
        let _=ctx.run(input(),|ctx|{manager.show(ctx,None,vec![],false);});
        let output=ctx.run(input(),|ctx|{manager.show(ctx,None,vec![],false);});
        let position=|label:&str|output.shapes.iter().find_map(|shape|match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text==label=>Some(text.pos.y),_=>None}).unwrap_or_else(||panic!("Missing {label}"));
        assert!(position("Active component")<position("1.0 / 2.0 MB"));
        assert!(position("1.0 / 2.0 MB")<position("Next component"));
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "d{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    % 0xffffffffff
            ));
            fs::create_dir_all(&path).unwrap();
            TEST_ROOT.with(|r| *r.borrow_mut() = Some(path.clone()));
            Self(path)
        }
        fn payload(&self, name: &str) -> PathBuf {
            let path = component_root(Component::Denoise)
                .join("versions")
                .join(name);
            fs::create_dir_all(path.join("models-cunet")).unwrap();
            fs::write(path.join(Component::Denoise.marker()), name).unwrap();
            for n in 0..=3 {
                for ext in ["bin", "param"] {
                    fs::write(
                        path.join(format!("models-cunet/noise{n}_model.{ext}")),
                        b"model",
                    )
                    .unwrap();
                }
            }
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            TEST_ROOT.with(|r| *r.borrow_mut() = None);
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn activation_and_rollback_switch_complete_versions_without_overwrite() {
        let f = Fixture::new();
        let a = f.payload("a");
        let b = f.payload("b");
        activate(Component::Denoise, &a, "a").unwrap();
        activate(Component::Denoise, &b, "b").unwrap();
        assert_eq!(active_path(Component::Denoise), Some(b.clone()));
        rollback(Component::Denoise).unwrap();
        assert_eq!(active_path(Component::Denoise), Some(a.clone()));
        rollback(Component::Denoise).unwrap();
        assert_eq!(active_path(Component::Denoise), Some(b.clone()));
        assert_eq!(fs::read(a.join(Component::Denoise.marker())).unwrap(), b"a");
    }
    #[test]
    fn incomplete_or_outside_install_never_replaces_active_version() {
        let f = Fixture::new();
        let a = f.payload("a");
        activate(Component::Denoise, &a, "a").unwrap();
        let empty = component_root(Component::Denoise).join("versions/empty");
        fs::create_dir_all(&empty).unwrap();
        assert!(activate(Component::Denoise, &empty, "bad").is_err());
        let outside = f.0.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join(Component::Denoise.marker()), b"x").unwrap();
        assert!(activate(Component::Denoise, &outside, "bad").is_err());
        assert_eq!(active_path(Component::Denoise), Some(a));
        safe_cleanup(Component::Denoise, &outside);
        assert!(outside.exists());
    }
    #[test]
    fn missing_rollback_and_cancel_leave_active_install_unchanged() {
        let f = Fixture::new();
        let a = f.payload("a");
        let b = f.payload("b");
        activate(Component::Denoise, &a, "a").unwrap();
        activate(Component::Denoise, &b, "b").unwrap();
        fs::remove_dir_all(a).unwrap();
        assert!(rollback(Component::Denoise).is_err());
        let transfer = Transfer::default();
        transfer.cancel.store(true, Ordering::Relaxed);
        assert!(
            install(Component::Denoise, "nvidia", &transfer)
                .unwrap_err()
                .contains("Cancelled")
        );
        assert_eq!(active_path(Component::Denoise), Some(b));
    }
    #[test]
    #[ignore = "Downloads official packages into a disposable isolated fixture"]
    fn official_packages_stage_and_deblur_without_changing_active_installations() {
        let f = Fixture::new();
        let transfer = Transfer::default();
        for c in [
            Component::Denoise,
            Component::Upscale,
            Component::DeblurModel,
            Component::DeblurRuntime,
        ] {
            println!("Staging {}", c.title());
            let stage = f.0.join(c.id());
            fs::create_dir(&stage).unwrap();
            let (payload, version) = install_stage(c, "nvidia", &stage, &transfer).unwrap();
            assert!(payload.join(c.marker()).is_file());
            assert!(!version.is_empty());
            assert!(!component_root(c).join("active.json").exists());
            if c == Component::DeblurRuntime {
                let model =
                    f.0.join(Component::DeblurModel.id())
                        .join(Component::DeblurModel.marker());
                let input = f.0.join("input.png");
                let output = f.0.join("output.png");
                image::RgbaImage::from_pixel(128, 128, image::Rgba([40, 60, 90, 128]))
                    .save(&input)
                    .unwrap();
                run(
                    Command::new(payload.join("python.exe"))
                        .arg(
                            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                .join("tools/restoration/nafnet_deblur.py"),
                        )
                        .arg("--input")
                        .arg(input)
                        .arg("--output")
                        .arg(&output)
                        .arg("--model")
                        .arg(model),
                    &f.0.join("inference.log"),
                    &transfer,
                )
                .unwrap();
                assert_eq!(
                    image::open(output).unwrap().into_rgba8().dimensions(),
                    (128, 128)
                );
            }
        }
    }

    #[test]
    fn only_missing_required_components_trigger_menu_badge() {
        let mut manager = Manager::default();
        manager.rows = vec![Status {
            component: None,
            title: "Qwen (optional)".into(),
            required: false,
            installed: false,
            detail: String::new(),
            rollback: false,
        }];
        assert_eq!(manager.missing(), 0);
        manager.rows.push(Status {
            component: Some(Component::DeblurModel),
            title: "Deblur".into(),
            required: true,
            installed: false,
            detail: String::new(),
            rollback: false,
        });
        assert_eq!(manager.missing(), 1);
        manager.rows[1].installed = true;
        assert_eq!(manager.missing(), 0);
    }
}
