use std::path::PathBuf;

pub fn is_dev() -> bool {
    std::env::var_os("PIXELFORGE_DEV").is_some_and(|value| value == "1")
}

pub fn settings_root() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join(if is_dev() {"PixelForgeDev"} else {"PixelForge"}))
}

pub fn window_title() -> String {
    format!("PixelForge{} v{}", if is_dev() {" Dev"} else {""}, env!("CARGO_PKG_VERSION"))
}
