//! Per-profile backend selection; never rewrite another environment or active registry.
use serde_json::json;
use std::path::PathBuf;
pub struct Config {
    pub mode: String,
    pub processing: String,
    pub modern: Option<PathBuf>,
    pub legacy: Option<PathBuf>,
}
impl Config {
    pub fn load() -> Self {
        let value = crate::profile::settings_root()
            .and_then(|p| std::fs::read(p.join("backend.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .unwrap_or_default();
        let mode = value["mode"]
            .as_str()
            .filter(|s| ["auto", "modern", "legacy"].contains(s))
            .unwrap_or("auto")
            .into();
        Self {
            mode,
            processing: value["processing"]
                .as_str()
                .filter(|value| ["auto", "gpu", "cpu"].contains(value))
                .unwrap_or("auto")
                .into(),
            modern: value["modern"].as_str().map(PathBuf::from),
            legacy: value["legacy"].as_str().map(PathBuf::from),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        let root = crate::profile::settings_root().ok_or("APPDATA unavailable")?;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let temp = root.join("backend.json.new");
        std::fs::write(
            &temp,
            serde_json::to_vec_pretty(
                &json!({"mode":self.mode,"processing":self.processing,"modern":self.modern,"legacy":self.legacy}),
            )
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temp, root.join("backend.json")).map_err(|e| e.to_string())
    }
    pub fn resolve(&self) -> Result<PathBuf, String> {
        let path=match self.mode.as_str() {
            "modern"=>self.modern.clone(),
            "legacy"=>self.legacy.clone(),
            _=>crate::dependencies::active_path(crate::dependencies::Component::Comfy).or_else(crate::dependencies::desktop_backend),
        }.ok_or("No backend configured. Open Setup > Setup & Diagnostics: keep Auto for the existing installation, or register a separate Modern / Legacy ComfyUI folder.")?;
        if !path.join("ComfyUI/main.py").is_file() || crate::compatibility::python(&path).is_none()
        {
            return Err("Selected backend is incomplete. Choose its portable root (containing ComfyUI and python_embeded), or the Desktop instance root. No fallback was silently selected.".into());
        }
        Ok(path)
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, enabled: bool) -> Result<(), String> {
        let before = self.mode.clone();
        let previous_processing = self.processing.clone();
        let mut selected = None;
        ui.add_enabled_ui(enabled, |ui| {
            ui.label("ComfyUI processing device");
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [("auto", "Auto"), ("gpu", "GPU"), ("cpu", "CPU")] {
                    ui.selectable_value(&mut self.processing, value.into(), label);
                }
            });
            ui.label("CPU is a fallback and can be very slow. Reconnect to apply a device change.");
            ui.label("Backend used for the next local launch");
            egui::ComboBox::from_id_salt("backend_mode")
                .selected_text(match self.mode.as_str() {
                    "modern" => "Modern (RTX)",
                    "legacy" => "Legacy (GTX 10-series)",
                    _ => "Auto (existing installation)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.mode,
                        "auto".into(),
                        "Auto (existing installation)",
                    );
                    ui.selectable_value(&mut self.mode, "modern".into(), "Modern (RTX)");
                    ui.selectable_value(
                        &mut self.mode,
                        "legacy".into(),
                        "Legacy (GTX 10-series, unverified)",
                    );
                });
            if self.mode != "auto" {
                let path = if self.mode == "legacy" {
                    &self.legacy
                } else {
                    &self.modern
                };
                ui.label(
                    path.as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or("No separate folder selected".into()),
                );
                if ui.button("Choose existing ComfyUI folder…").clicked() {
                    selected = rfd::FileDialog::new()
                        .set_title("Select ComfyUI portable or Desktop instance root")
                        .pick_folder();
                }
            }
        });
        if let Some(path) = selected {
            if !path.join("ComfyUI/main.py").is_file()
                || crate::compatibility::python(&path).is_none()
            {
                return Err("That folder has no complete ComfyUI / Python installation".into());
            }
            if self.mode == "legacy" {
                self.legacy = Some(path);
            } else {
                self.modern = Some(path);
            }
            self.save()?;
        } else if self.mode != before || self.processing != previous_processing {
            self.save()?;
        }
        if self.processing != previous_processing {
            crate::ai_generation::stop_managed_backend();
        }
        ui.label("Reconnect after changing the processing device. If you run ComfyUI separately, restart it with the selected device.");
        ui.label("Legacy: use a separate official CUDA 12.6 portable package. FP8 / newer models can still be unsupported. GTX 1080 generation awaits hardware testing.");
        ui.hyperlink_to(
            "Official ComfyUI portable packages",
            "https://docs.comfy.org/installation/comfyui_portable_windows",
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_missing_backends_never_fall_back_to_shared_environment() {
        for mode in ["legacy", "modern"] {
            let c = Config {
                mode: mode.into(),
                processing: "auto".into(),
                modern: None,
                legacy: None,
            };
            assert!(c.resolve().is_err());
        }
    }
}
