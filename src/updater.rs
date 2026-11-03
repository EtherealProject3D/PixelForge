//! Explicit, stable-release updates from the project's own GitHub repository.
use std::{fs, io::{Read, Write}, path::PathBuf, process::{Command, Stdio}, sync::{Arc, atomic::{AtomicU64, Ordering}, mpsc}, time::Duration};
use serde_json::Value;
const API: &str = "https://api.github.com/repos/EtherealProject3D/PixelForge/releases/latest";
const DOWNLOADS: &str = "https://github.com/EtherealProject3D/PixelForge/releases/download/";

#[derive(Clone)]
struct Release { tag: String, notes: String, zip: String, checksum: String, asset: String, setup: Option<String> }
impl Release {
    fn for_installed_copy(mut self, installed:bool)->Result<Self,String> {
        if installed {
            self.zip=self.setup.clone().ok_or("This release has no setup package. Please download it from GitHub manually.".to_owned())?;
            self.asset=format!("PixelForge-{}-windows-x64-setup.exe",self.tag);
        }
        Ok(self)
    }
}
fn installed_copy()->bool {
    std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("pixelforge-installed.txt").is_file())).unwrap_or(false)
}
fn version(value: &str) -> Option<[u32;3]> {
    let parts: Vec<_> = value.trim_start_matches('v').split('.').collect();
    if parts.len()!=3 {return None;}
    Some([parts[0].parse().ok()?,parts[1].parse().ok()?,parts[2].parse().ok()?])
}
fn parse_release(value: &Value) -> Result<Option<Release>,String> {
    if value["draft"].as_bool().unwrap_or(true) || value["prerelease"].as_bool().unwrap_or(true) {return Ok(None);}
    let tag=value["tag_name"].as_str().ok_or("Release has no version")?;
    let remote=version(tag).ok_or("Release version is not supported")?;
    if remote<=version(env!("CARGO_PKG_VERSION")).unwrap() {return Ok(None);}
    let asset=format!("PixelForge-{tag}-windows-x64.zip");
    let find=|name:&str| -> Result<String,String> {
        let url=value["assets"].as_array().and_then(|a|a.iter().find(|a|a["name"].as_str()==Some(name)))
            .and_then(|a|a["browser_download_url"].as_str()).ok_or_else(||format!("Release is missing {name}"))?;
        if url!=format!("{DOWNLOADS}{tag}/{name}") {return Err("Release asset is outside the official download location".into());}
        Ok(url.into())
    };
    let setup_name=format!("PixelForge-{tag}-windows-x64-setup.exe");
    let setup=if value["assets"].as_array().is_some_and(|a|a.iter().any(|a|a["name"].as_str()==Some(&setup_name))) {Some(find(&setup_name)?)} else {None};
    Ok(Some(Release {tag:tag.into(),notes:value["body"].as_str().unwrap_or("").into(),zip:find(&asset)?,checksum:find("SHA256SUMS.txt")?,asset,setup}))
}
fn client() -> Result<reqwest::blocking::Client,String> {
    reqwest::blocking::Client::builder().user_agent(concat!("PixelForge/",env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(600)).build().map_err(|e|e.to_string())
}
fn checksum(text:&str, asset:&str) -> Result<String,String> {
    for line in text.lines() {
        let parts:Vec<_>=line.split_whitespace().collect();
        if parts.len()==2 && parts[1].trim_start_matches('*')==asset && parts[0].len()==64 && parts[0].bytes().all(|b|b.is_ascii_hexdigit()) {return Ok(parts[0].to_ascii_lowercase());}
    }
    Err("Published checksum is missing or invalid".into())
}
fn hidden(command:&mut Command) {
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
}
fn stage(release:Release, progress:Arc<AtomicU64>, total:Arc<AtomicU64>) -> Result<PathBuf,String> {
    let client=client()?;
    let mut checks=String::new();
    client.get(&release.checksum).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?
        .take(64*1024).read_to_string(&mut checks).map_err(|e|e.to_string())?;
    let hash=checksum(&checks,&release.asset)?;
    let root=PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?).join("PixelForge/updates")
        .join(format!("{}-{}",release.tag,std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()));
    fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let archive=root.join("update.zip");
    let mut response=client.get(&release.zip).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?;
    let length=response.content_length().ok_or("Download size is unavailable")?;
    if length==0 || length>150_000_000 {return Err("Unexpected update download size".into());}
    total.store(length,Ordering::Relaxed);
    let mut output=fs::File::create(&archive).map_err(|e|e.to_string())?;
    let mut buffer=[0u8;65536];
    let mut downloaded=0u64;
    loop {
        let n=response.read(&mut buffer).map_err(|e|e.to_string())?;
        if n==0 {break;}
        downloaded+=n as u64;
        if downloaded>length {return Err("Update exceeded expected size".into());}
        output.write_all(&buffer[..n]).map_err(|e|e.to_string())?;
        progress.store(downloaded,Ordering::Relaxed);
    }
    if downloaded!=length {return Err("Update download was incomplete".into());}
    drop(output);
    let setup=release.asset.ends_with("-setup.exe");
    let staged=root.join(if setup {"PixelForge-Setup.exe"} else {"PixelForge.exe"});
    let script=root.join("verify.ps1");
    fs::write(&script,include_str!("../scripts/Stage-Update.ps1")).map_err(|e|e.to_string())?;
    let mut command=Command::new("powershell.exe");
    command.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(script)
        .env("PF_UPDATE_ZIP",&archive).env("PF_UPDATE_HASH",hash).env("PF_UPDATE_STAGED",&staged)
        .env("PF_UPDATE_VERSION",release.tag.trim_start_matches('v')).env("PF_UPDATE_SETUP",if setup {"1"} else {"0"});
    hidden(&mut command);
    let result=command.output().map_err(|e|e.to_string())?;
    if !result.status.success() {return Err(format!("Update verification failed: {}",String::from_utf8_lossy(&result.stderr)));}
    Ok(staged)
}
pub struct Manager {
    pub open: bool,
    message: String,
    release: Option<Release>,
    staged: Option<PathBuf>,
    check_rx: Option<mpsc::Receiver<Result<Option<Release>,String>>>,
    download_rx: Option<mpsc::Receiver<Result<PathBuf,String>>>,
    bytes: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
}
impl Default for Manager {
    fn default()->Self {Self {open:false,message:"Check for a newer published release.".into(),release:None,staged:None,check_rx:None,download_rx:None,bytes:Arc::new(AtomicU64::new(0)),total:Arc::new(AtomicU64::new(0))}}
}
impl Manager {
    pub fn busy(&self)->bool {self.check_rx.is_some()||self.download_rx.is_some()}
    pub fn show(&mut self,ctx:&egui::Context,can_restart:bool)->bool {
        if let Some(result)=self.check_rx.as_ref().and_then(|r|r.try_recv().ok()) {
            self.check_rx=None;
            match result {Ok(release)=>{self.message=if release.is_some(){"A new version is available."}else{"You have the latest published version."}.into();self.release=release;},Err(e)=>self.message=format!("Could not check for updates: {e}")}
        }
        if let Some(result)=self.download_rx.as_ref().and_then(|r|r.try_recv().ok()) {
            self.download_rx=None;
            match result {Ok(path)=>{self.staged=Some(path);self.message="Download verified. Ready to install and restart.".into();},Err(e)=>self.message=e}
        }
        if self.busy() {ctx.request_repaint_after(Duration::from_millis(100));}
        if !self.open {return false;}
        let mut open=true;
        let mut restart=false;
        egui::Window::new("PixelForge Updates").open(&mut open).default_width(520.0).min_width(440.0)
            .default_pos(ctx.screen_rect().center()-egui::vec2(260.0,160.0)).resizable(true).show(ctx,|ui| {
            ui.heading(format!("PixelForge v{}",env!("CARGO_PKG_VERSION")));
            ui.label("Updates come from published PixelForge GitHub releases.");
            if ui.add_enabled(!self.busy(),egui::Button::new("Check for Updates")).clicked() {
                self.release=None;self.staged=None;self.message="Checking GitHub Releases…".into();
                let (tx,rx)=mpsc::channel();self.check_rx=Some(rx);
                std::thread::spawn(move|| {let result=(|| {let value:Value=client()?.get(API).send().map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?.json().map_err(|e|e.to_string())?;parse_release(&value)})();let _=tx.send(result);});
            }
            ui.label(&self.message);
            if self.busy() {ui.spinner();}
            if let Some(release)=self.release.clone() {
                ui.strong(format!("Available: {}",release.tag));
                egui::ScrollArea::vertical().max_height(220.0).show(ui,|ui| {ui.label(&release.notes);});
                if self.staged.is_none() && ui.add_enabled(!self.busy(),crate::theme::primary("Download Update")).clicked() {
                    self.bytes.store(0,Ordering::Relaxed);self.total.store(0,Ordering::Relaxed);
                    let (tx,rx)=mpsc::channel();self.download_rx=Some(rx);self.message="Downloading and verifying update…".into();
                    let (bytes,total)=(self.bytes.clone(),self.total.clone());
                    let installed=installed_copy();
                    std::thread::spawn(move|| {let _=tx.send(release.for_installed_copy(installed).and_then(|release|stage(release,bytes,total)));});
                }
            }
            if self.download_rx.is_some() {
                let (bytes,total)=(self.bytes.load(Ordering::Relaxed),self.total.load(Ordering::Relaxed));
                ui.add(egui::ProgressBar::new(if total>0{bytes as f32/total as f32}else{0.0}).text(format!("{:.1} / {:.1} MB",bytes as f64/1e6,total as f64/1e6)));
            }
            if self.staged.is_some() {
                if installed_copy() {ui.label("The setup wizard will ask Windows for administrator permission. Finish setup to reopen PixelForge.");}
                if !can_restart {ui.label("Save all open projects and finish running operations before installing.");}
                if ui.add_enabled(can_restart,crate::theme::primary("Install & Restart")).clicked() {
                    match self.install() {Ok(())=>restart=true,Err(e)=>self.message=e}
                }
            }
            ui.hyperlink_to("View releases on GitHub","https://github.com/EtherealProject3D/PixelForge/releases");
        });
        self.open=open;
        restart
    }
    fn install(&self)->Result<(),String> {
        let staged=self.staged.as_ref().ok_or("No update is ready")?;
        if staged.file_name().is_some_and(|name|name=="PixelForge-Setup.exe") {
            let mut command=Command::new("powershell.exe");
            command.args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; try {Start-Process -FilePath $env:PF_SETUP_PATH -Verb RunAs; exit 0} catch {Write-Error $_; exit 1}"])
                .env("PF_SETUP_PATH",staged);
            hidden(&mut command);
            let result=command.output().map_err(|e|e.to_string())?;
            if !result.status.success() {return Err("Setup did not start or administrator permission was cancelled. PixelForge is still open.".into());}
            return Ok(());
        }
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        let folder=exe.parent().ok_or("No installation folder")?;
        let probe=folder.join(format!(".pixelforge-update-write-test-{}",std::process::id()));
        fs::write(&probe,b"").map_err(|_|"This installation folder is not writable. Download the update from GitHub and extract it into a writable folder.".to_owned())?;
        fs::remove_file(probe).map_err(|e|e.to_string())?;
        let root=staged.parent().ok_or("No staging folder")?;
        let helper=root.join("install.ps1");
        fs::write(&helper,include_str!("../scripts/Install-Update.ps1")).map_err(|e|e.to_string())?;
        let mut command=Command::new("powershell.exe");
        command.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(helper)
            .env("PF_UPDATE_PID",std::process::id().to_string()).env("PF_UPDATE_TARGET",&exe)
            .env("PF_UPDATE_STAGED",staged).env("PF_UPDATE_WORKDIR",folder).env("PF_UPDATE_LOG",root.join("install-error.log"));
        hidden(&mut command);
        command.spawn().map_err(|e|e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_newer_stable_official_complete_releases_are_offered() {
        let mut release=serde_json::json!({"tag_name":"v9.2.1","draft":false,"prerelease":false,"body":"Notes","assets":[
            {"name":"PixelForge-v9.2.1-windows-x64.zip","browser_download_url":format!("{DOWNLOADS}v9.2.1/PixelForge-v9.2.1-windows-x64.zip")},
            {"name":"SHA256SUMS.txt","browser_download_url":format!("{DOWNLOADS}v9.2.1/SHA256SUMS.txt")}]});
        assert!(parse_release(&release).unwrap().is_some());
        assert!(parse_release(&release).unwrap().unwrap().for_installed_copy(true).is_err());
        release["assets"].as_array_mut().unwrap().push(serde_json::json!({"name":"PixelForge-v9.2.1-windows-x64-setup.exe","browser_download_url":format!("{DOWNLOADS}v9.2.1/PixelForge-v9.2.1-windows-x64-setup.exe")}));
        let installed=parse_release(&release).unwrap().unwrap().for_installed_copy(true).unwrap();
        assert!(installed.zip.ends_with("-setup.exe"));
        assert_eq!(installed.asset,"PixelForge-v9.2.1-windows-x64-setup.exe");
        assert!(parse_release(&release).unwrap().unwrap().for_installed_copy(false).unwrap().asset.ends_with(".zip"));
        release["prerelease"]=true.into();assert!(parse_release(&release).unwrap().is_none());
        release["prerelease"]=false.into();release["assets"][0]["browser_download_url"]="https://example.com/app.exe".into();assert!(parse_release(&release).is_err());
        release["tag_name"]="v0.6.0".into();assert!(parse_release(&release).unwrap().is_none());
        assert!(version("v1.2.3-beta").is_none());
        assert_eq!(version("v0.10.0"),Some([0,10,0]));
    }
    #[test]
    fn checksum_must_match_the_exact_asset() {
        let hash="a".repeat(64);
        assert_eq!(checksum(&format!("{hash}  app.zip"),"app.zip").unwrap(),hash);
        assert!(checksum(&format!("{hash}  other.zip"),"app.zip").is_err());
        assert!(checksum("not-a-hash app.zip","app.zip").is_err());
    }
}
