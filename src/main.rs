mod adjustments;
mod project;

use eframe::egui;
use egui::{Color32, ColorImage, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PanButton {
    Middle,
    Left,
    SpaceLeft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FitButton {
    Left,
    Middle,
}

struct Controls {
    pan: PanButton,
    fit: FitButton,
    zoom_sensitivity: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            pan: PanButton::Middle,
            fit: FitButton::Left,
            zoom_sensitivity: 1.0,
        }
    }
}

impl Controls {
    fn config_path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(base).join("PixelForge").join("controls.cfg"))
    }

    fn load() -> Self {
        let mut settings = Self::default();

        let Some(path) = Self::config_path() else {
            return settings;
        };

        let Ok(content) = std::fs::read_to_string(path) else {
            return settings;
        };

        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match key.trim() {
                "pan" => {
                    settings.pan = match value.trim() {
                        "left" => PanButton::Left,
                        "space_left" => PanButton::SpaceLeft,
                        _ => PanButton::Middle,
                    };
                }
                "fit" => {
                    settings.fit = match value.trim() {
                        "middle" => FitButton::Middle,
                        _ => FitButton::Left,
                    };
                }
                "zoom_sensitivity" => {
                    if let Ok(value) = value.trim().parse::<f32>() {
                        if value.is_finite() {
                            settings.zoom_sensitivity = value.clamp(0.25, 3.0);
                        }
                    }
                }
                _ => {}
            }
        }

        settings
    }

    fn save(&self) -> std::io::Result<()> {
        let path =
            Self::config_path().ok_or_else(|| std::io::Error::other("APPDATA is unavailable"))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let pan = match self.pan {
            PanButton::Middle => "middle",
            PanButton::Left => "left",
            PanButton::SpaceLeft => "space_left",
        };

        let fit = match self.fit {
            FitButton::Left => "left",
            FitButton::Middle => "middle",
        };

        let contents = format!(
            "pan={pan}\nfit={fit}\nzoom_sensitivity={}\n",
            self.zoom_sensitivity
        );

        std::fs::write(path, contents)
    }
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Png,
    Jpeg,
    WebP,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG Image",
            Self::Jpeg => "JPEG Image",
            Self::WebP => "WebP Image",
        }
    }
}

enum ExportEvent {
    Progress {
        fraction: f32,
        stage: &'static str,
        encoding: bool,
    },
    Finished(Result<PathBuf, String>),
}
#[derive(Clone, Copy)]
enum PendingOpen {
    Image,
    Project,
    Close,
}

struct PixelForge {
    texture: Option<TextureHandle>,
    processed_texture: Option<TextureHandle>,
    original_pixels: Option<image::RgbaImage>,
    brightness: f32,
    export_rx: Option<std::sync::mpsc::Receiver<ExportEvent>>,
    export_progress: f32,
    export_stage: String,
    export_encoding: bool,
    exporting: bool,
    export_result: Option<bool>,
    contrast: f32,
    exposure: f32,
    saturation: f32,
    temperature: f32,
    tint: f32,
    comparison_enabled: bool,
    comparison_position: f32,
    divider_dragging: bool,
    image_path: Option<PathBuf>,
    image_size: [usize; 2],
    file_size: u64,
    zoom: f32,
    offset: Vec2,
    fit_pending: bool,
    panning: bool,
    advanced: bool,
    show_settings: bool,
    controls: Controls,
    status: String,
    project_path: Option<PathBuf>,
    project_dirty: bool,
    pending_open: Option<PendingOpen>,
    exit_confirmation: bool,
    exit_allowed: bool,
}

impl Default for PixelForge {
    fn default() -> Self {
        Self {
            texture: None,
            processed_texture: None,
            original_pixels: None,
            brightness: 0.0,
            export_rx: None,
            export_progress: 0.0,
            export_stage: String::new(),
            export_encoding: false,
            exporting: false,
            export_result: None,
            contrast: 0.0,
            exposure: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            tint: 0.0,
            comparison_enabled: true,
            comparison_position: 0.5,
            divider_dragging: false,
            image_path: None,
            image_size: [0, 0],
            file_size: 0,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_pending: false,
            panning: false,
            advanced: false,
            show_settings: false,
            controls: Controls::load(),
            status: "Ready".into(),
            project_path: None,
            project_dirty: false,
            pending_open: None,
            exit_confirmation: false,
            exit_allowed: false,
        }
    }
}

impl PixelForge {
    fn open_image(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "tiff"])
            .pick_file()
        else {
            return;
        };

        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];

                let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

                self.texture =
                    Some(ctx.load_texture("current_image", color_image, TextureOptions::LINEAR));

                self.file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                self.original_pixels = Some(rgba);
                self.project_path = None;
                self.project_dirty = true;
                self.export_result = None;
                self.export_progress = 0.0;
                self.processed_texture = None;
                self.brightness = 0.0;
                self.contrast = 0.0;
                self.exposure = 0.0;
                self.saturation = 0.0;
                self.temperature = 0.0;
                self.tint = 0.0;
                self.comparison_position = 0.5;
                self.comparison_enabled = true;
                self.divider_dragging = false;
                self.image_size = size;
                self.image_path = Some(path);
                self.fit_pending = true;
                self.offset = Vec2::ZERO;
                self.panning = false;
                self.status = "Image loaded".into();
            }
            Err(err) => {
                self.status = format!("Open failed: {err}");
            }
        }
    }

    fn request_open(&mut self, action: PendingOpen, ctx: &egui::Context) {
        if self.project_dirty {
            self.pending_open = Some(action);
        } else {
            self.perform_open(action, ctx);
        }
    }

    fn perform_open(&mut self, action: PendingOpen, ctx: &egui::Context) {
        match action {
            PendingOpen::Image => self.open_image(ctx),
            PendingOpen::Project => self.open_project(ctx),
            PendingOpen::Close => self.close_image(),
        }
    }

    fn close_image(&mut self) {
        // Release image textures and original image data.
        self.texture = None;
        self.processed_texture = None;
        self.original_pixels = None;

        // Clear image and project references.
        self.image_path = None;
        self.project_path = None;
        self.project_dirty = false;
        self.pending_open = None;

        // Reset adjustments and comparison.
        self.brightness = 0.0;
        self.contrast = 0.0;
        self.exposure = 0.0;
        self.saturation = 0.0;
        self.temperature = 0.0;
        self.tint = 0.0;
        self.comparison_enabled = true;
        self.comparison_position = 0.5;
        self.divider_dragging = false;

        // Reset viewer state.
        self.image_size = [0, 0];
        self.file_size = 0;
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
        self.fit_pending = false;
        self.panning = false;

        // Clear completed export information.
        self.export_rx = None;
        self.export_progress = 0.0;
        self.export_stage.clear();
        self.export_encoding = false;
        self.export_result = None;

        self.status = "Ready - open an image or project".into();
    }
    fn show_exit_confirmation(&mut self, ctx: &egui::Context) {
        if !self.exit_confirmation {
            return;
        }

        egui::Window::new("Unsaved Changes - Exit PixelForge")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Your current image or project has unsaved changes.");
                ui.label("What would you like to do before exiting?");

                ui.add_space(10.0);

                if ui.button("Save Project & Exit").clicked() {
                    self.save_project(false);

                    // Only exit after a successful save.
                    // Canceled or failed saves leave the app open.
                    if !self.project_dirty {
                        self.exit_confirmation = false;
                        self.exit_allowed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.exit_confirmation = false;
                    }

                    if ui.button("Discard Changes & Exit").clicked() {
                        self.exit_confirmation = false;
                        self.exit_allowed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
    }
    fn show_unsaved_prompt(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_open else {
            return;
        };

        egui::Window::new("Unsaved Project Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("This image has unsaved project changes.");
                match action {
                    PendingOpen::Close => {
                        ui.label("Closing this image will discard those changes.");
                    }
                    _ => {
                        ui.label("Opening another file will discard those changes.");
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_open = None;
                    }

                    if ui.button("Discard Changes").clicked() {
                        self.pending_open = None;
                        self.perform_open(action, ctx);
                    }
                });
            });
    }

    fn save_project(&mut self, save_as: bool) {
        let Some(original) = &self.original_pixels else {
            self.status = "Open an image first".into();
            return;
        };

        let path = if save_as {
            None
        } else {
            self.project_path.clone()
        };

        let path = match path {
            Some(path) => path,
            None => {
                let stem = self
                    .image_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into());

                let suggested = format!("{stem}.pxf");

                let Some(mut path) = rfd::FileDialog::new()
                    .add_filter("PixelForge Project", &["pxf"])
                    .set_file_name(&suggested)
                    .save_file()
                else {
                    return;
                };

                path.set_extension("pxf");
                path
            }
        };

        let image_name = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image.png".into());

        let data = project::Project {
            pixels: original.clone(),
            image_name,
            brightness: self.brightness,
            contrast: self.contrast,
            exposure: self.exposure,
            saturation: self.saturation,
            temperature: self.temperature,
            tint: self.tint,
            comparison_enabled: self.comparison_enabled,
            comparison_position: self.comparison_position,
        };

        match project::save(&path, &data) {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.project_dirty = false;
                self.status = format!("Project saved: {}", path.display());
            }
            Err(err) => {
                self.status = format!("Project save failed: {err}");
            }
        }
    }

    fn open_project(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PixelForge Project", &["pxf"])
            .pick_file()
        else {
            return;
        };

        // Load and validate before changing the current image.
        let data = match project::load(&path) {
            Ok(data) => data,
            Err(err) => {
                self.status = format!("Project open failed: {err}");
                return;
            }
        };

        let size = [data.pixels.width() as usize, data.pixels.height() as usize];

        let color = ColorImage::from_rgba_unmultiplied(size, data.pixels.as_raw());

        self.texture = Some(ctx.load_texture("current_image", color, TextureOptions::LINEAR));

        self.original_pixels = Some(data.pixels);
        self.processed_texture = None;

        self.brightness = data.brightness;
        self.contrast = data.contrast;
        self.exposure = data.exposure;
        self.saturation = data.saturation;
        self.temperature = data.temperature;
        self.tint = data.tint;
        self.comparison_enabled = data.comparison_enabled;
        self.comparison_position = data.comparison_position;

        self.image_size = size;
        self.image_path = Some(PathBuf::from(data.image_name));
        self.file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        self.project_path = Some(path.clone());
        self.project_dirty = false;

        self.fit_pending = true;
        self.offset = Vec2::ZERO;
        self.panning = false;
        self.divider_dragging = false;

        self.export_result = None;
        self.export_progress = 0.0;
        self.export_stage.clear();

        if self.brightness != 0.0
            || self.contrast != 0.0
            || self.exposure != 0.0
            || self.saturation != 0.0
            || self.temperature != 0.0
            || self.tint != 0.0
        {
            self.update_processed_image(ctx);
        }

        self.status = format!("Project opened: {}", path.display());
    }
    fn fit_image(&mut self, available: Vec2) {
        let w = self.image_size[0] as f32;
        let h = self.image_size[1] as f32;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        self.zoom = (available.x / w).min(available.y / h).clamp(0.001, 16.0);

        self.offset = Vec2::ZERO;
        self.fit_pending = false;
    }

    fn zoom_at(&mut self, factor: f32, anchor: Pos2, center: Pos2) {
        let old_zoom = self.zoom;
        let new_zoom = (old_zoom * factor).clamp(0.001, 16.0);

        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        let anchor_offset = anchor - center;
        let ratio = new_zoom / old_zoom;

        self.offset = anchor_offset - (anchor_offset - self.offset) * ratio;

        self.zoom = new_zoom;
    }

    fn save_controls(&mut self) {
        match self.controls.save() {
            Ok(()) => {
                self.status = "Control settings saved".into();
            }
            Err(err) => {
                self.status = format!("Settings save failed: {err}");
            }
        }
    }

    fn show_controls_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let mut open = self.show_settings;
        let mut changed = false;

        egui::Window::new("Settings - Controls")
            .open(&mut open)
            .default_width(390.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Mouse Controls");
                ui.separator();

                ui.label("Pan image");

                changed |= ui
                    .radio_value(
                        &mut self.controls.pan,
                        PanButton::Middle,
                        "Middle-click + drag",
                    )
                    .changed();

                changed |= ui
                    .radio_value(&mut self.controls.pan, PanButton::Left, "Left-click + drag")
                    .changed();

                changed |= ui
                    .radio_value(
                        &mut self.controls.pan,
                        PanButton::SpaceLeft,
                        "Space + left-click + drag",
                    )
                    .changed();

                ui.add_space(10.0);
                ui.label("Fit image to window");

                changed |= ui
                    .radio_value(
                        &mut self.controls.fit,
                        FitButton::Left,
                        "Left-button double-click",
                    )
                    .changed();

                changed |= ui
                    .radio_value(
                        &mut self.controls.fit,
                        FitButton::Middle,
                        "Middle-button double-click",
                    )
                    .changed();

                ui.add_space(10.0);
                ui.label("Mouse wheel zoom sensitivity");

                changed |= ui
                    .add(
                        egui::Slider::new(&mut self.controls.zoom_sensitivity, 0.25..=3.0)
                            .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
                    )
                    .changed();

                ui.add_space(10.0);
                ui.separator();

                if ui.button("Restore Default Controls").clicked() {
                    self.controls = Controls::default();
                    changed = true;
                }

                ui.label("Changes are saved automatically.");
            });

        self.show_settings = open;

        if changed {
            self.panning = false;
            self.save_controls();
        }
    }

    fn update_processed_image(&mut self, ctx: &egui::Context) {
        let Some(original) = &self.original_pixels else {
            return;
        };

        let mut adjusted = original.clone();

        adjustments::Adjustments {
            brightness: self.brightness,
            contrast: self.contrast,
            exposure: self.exposure,
            saturation: self.saturation,
            temperature: self.temperature,
            tint: self.tint,
        }
        .apply(&mut adjusted);
        let size = [adjusted.width() as usize, adjusted.height() as usize];

        let color_image = ColorImage::from_rgba_unmultiplied(size, adjusted.as_raw());

        if let Some(texture) = &mut self.processed_texture {
            texture.set(color_image, TextureOptions::LINEAR);
        } else {
            self.processed_texture =
                Some(ctx.load_texture("processed_image", color_image, TextureOptions::LINEAR));
        }

        self.status = "Preview updated".into();
    }

    fn start_export(&mut self, format: ExportFormat) {
        if self.exporting {
            self.status = "An export is already running".into();
            return;
        }

        let Some(original) = &self.original_pixels else {
            self.status = "Open an image before exporting".into();
            return;
        };

        let stem = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_owned());

        let suggested = format!("{}_edited.{}", stem, format.extension());

        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(format.label(), &[format.extension()])
            .set_file_name(&suggested)
            .save_file()
        else {
            return;
        };

        // The selected menu option determines the output format.
        // Do not infer the format from a typed filename.
        path.set_extension(format.extension());

        // Filename collisions are resolved automatically
        // by the export worker using numbered suffixes.

        let original = original.clone();
        let brightness = self.brightness;
        let contrast = self.contrast;
        let exposure = self.exposure;
        let saturation = self.saturation;
        let temperature = self.temperature;
        let tint = self.tint;

        let (tx, rx) = std::sync::mpsc::channel();

        self.export_rx = Some(rx);
        self.export_progress = 0.0;
        self.export_stage = "Starting export".into();
        self.export_encoding = false;
        self.exporting = true;
        self.export_result = None;
        self.status = "Export started".into();

        std::thread::spawn(move || {
            let mut created_output = false;

            let result = (|| -> Result<(), String> {
                use std::io::{Cursor, Write};

                let mut adjusted = original;

                let settings = adjustments::Adjustments {
                    brightness,
                    contrast,
                    exposure,
                    saturation,
                    temperature,
                    tint,
                };

                let total = adjusted.width() as usize * adjusted.height() as usize;

                for (index, pixel) in adjusted.pixels_mut().enumerate() {
                    settings.apply_pixel(pixel);
                    if index % 250_000 == 0 && total > 0 {
                        let fraction = 0.70 * index as f32 / total as f32;

                        let _ = tx.send(ExportEvent::Progress {
                            fraction,
                            stage: "Applying adjustments",
                            encoding: false,
                        });
                    }
                }

                // The image crate does not provide an encoder
                // progress callback. Display an activity indicator
                // instead of pretending to measure its progress.
                let _ = tx.send(ExportEvent::Progress {
                    fraction: 0.70,
                    stage: "Encoding image",
                    encoding: true,
                });

                let bytes = match format {
                    ExportFormat::Jpeg => {
                        let rgb = image::DynamicImage::ImageRgba8(adjusted).to_rgb8();

                        let mut output = Vec::new();

                        {
                            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                                &mut output,
                                100,
                            );

                            encoder
                                .encode(
                                    rgb.as_raw(),
                                    rgb.width(),
                                    rgb.height(),
                                    image::ExtendedColorType::Rgb8,
                                )
                                .map_err(|e| e.to_string())?;
                        }

                        output
                    }

                    ExportFormat::Png | ExportFormat::WebP => {
                        let image = image::DynamicImage::ImageRgba8(adjusted);

                        let image_format = match format {
                            ExportFormat::Png => image::ImageFormat::Png,
                            ExportFormat::WebP => image::ImageFormat::WebP,
                            ExportFormat::Jpeg => unreachable!(),
                        };

                        let mut buffer = Cursor::new(Vec::new());

                        image
                            .write_to(&mut buffer, image_format)
                            .map_err(|e| e.to_string())?;

                        buffer.into_inner()
                    }
                };

                let _ = tx.send(ExportEvent::Progress {
                    fraction: 0.80,
                    stage: "Writing image file",
                    encoding: false,
                });

                // Create the output exclusively. If its name
                // exists, try _001, _002, and so on.
                // No existing file is ever opened for writing.
                let base_path = path.clone();

                let stem = base_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "image".to_owned());

                let extension = format.extension();

                let mut suffix = 0u32;

                let mut file = loop {
                    let candidate = if suffix == 0 {
                        base_path.clone()
                    } else {
                        base_path.with_file_name(format!("{stem}_{suffix:03}.{extension}"))
                    };

                    match std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&candidate)
                    {
                        Ok(file) => {
                            path = candidate;
                            break file;
                        }

                        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                            suffix = suffix
                                .checked_add(1)
                                .ok_or_else(|| "No available numbered filename".to_owned())?;
                        }

                        Err(err) => {
                            return Err(err.to_string());
                        }
                    }
                };

                created_output = true;

                // Actual file-writing progress: 80-99%.
                let total_bytes = bytes.len().max(1);
                let mut written = 0usize;

                for chunk in bytes.chunks(1024 * 1024) {
                    file.write_all(chunk).map_err(|e| e.to_string())?;

                    written += chunk.len();

                    let fraction = 0.80 + 0.19 * written as f32 / total_bytes as f32;

                    let _ = tx.send(ExportEvent::Progress {
                        fraction,
                        stage: "Writing image file",
                        encoding: false,
                    });
                }

                file.sync_all().map_err(|e| e.to_string())?;

                let _ = tx.send(ExportEvent::Progress {
                    fraction: 1.0,
                    stage: "Export complete",
                    encoding: false,
                });

                Ok(())
            })();

            // Remove only an incomplete file created by this
            // export worker. Never remove a pre-existing file.
            if result.is_err() && created_output {
                let _ = std::fs::remove_file(&path);
            }

            let final_result = result.map(|_| path);

            let _ = tx.send(ExportEvent::Finished(final_result));
        });
    }

    fn poll_export(&mut self, ctx: &egui::Context) {
        if !self.exporting {
            return;
        }

        let mut finished = false;

        loop {
            let event = self.export_rx.as_ref().and_then(|rx| rx.try_recv().ok());

            let Some(event) = event else {
                break;
            };

            match event {
                ExportEvent::Progress {
                    fraction,
                    stage,
                    encoding,
                } => {
                    self.export_progress = fraction.clamp(0.0, 1.0);
                    self.export_stage = stage.into();
                    self.export_encoding = encoding;
                }

                ExportEvent::Finished(result) => {
                    self.exporting = false;
                    self.export_result = Some(result.is_ok());
                    self.export_encoding = false;
                    finished = true;

                    match result {
                        Ok(path) => {
                            self.export_progress = 1.0;
                            self.export_stage = "Export complete".into();
                            self.status = format!("Exported: {}", path.display());
                        }
                        Err(err) => {
                            self.export_stage = "Export failed".into();
                            self.export_progress = 0.0;
                            self.status = format!("Export failed: {err}");
                        }
                    }
                }
            }
        }

        if finished {
            self.export_rx = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
    fn show_adjustments(&mut self, ui: &mut egui::Ui) {
        let before = (
            self.brightness,
            self.contrast,
            self.exposure,
            self.saturation,
            self.temperature,
            self.tint,
            self.comparison_enabled,
            self.comparison_position,
        );
        ui.horizontal(|ui| {
            ui.heading("Image Adjustments");

            let can_reset = self.brightness != 0.0
                || self.contrast != 0.0
                || self.exposure != 0.0
                || self.saturation != 0.0
                || self.temperature != 0.0
                || self.tint != 0.0;

            if ui
                .add_enabled(can_reset, egui::Button::new("Reset All"))
                .clicked()
            {
                self.brightness = 0.0;
                self.contrast = 0.0;
                self.exposure = 0.0;
                self.saturation = 0.0;
                self.temperature = 0.0;
                self.tint = 0.0;
                self.update_processed_image(ui.ctx());
            }
        });

        ui.separator();

        let mut changed = false;

        ui.label("Brightness");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.brightness, -100.0..=100.0))
                .changed();

            if ui
                .add_enabled(self.brightness != 0.0, egui::Button::new("Reset"))
                .on_hover_text("Reset brightness only")
                .clicked()
            {
                self.brightness = 0.0;
                changed = true;
            }
        });

        ui.add_space(6.0);

        ui.label("Contrast");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.contrast, -100.0..=100.0))
                .changed();

            if ui
                .add_enabled(self.contrast != 0.0, egui::Button::new("Reset"))
                .on_hover_text("Reset contrast only")
                .clicked()
            {
                self.contrast = 0.0;
                changed = true;
            }
        });

        ui.add_space(6.0);

        ui.label("Exposure (EV)");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.exposure, -3.0..=3.0).step_by(0.1))
                .changed();

            if ui
                .add_enabled(self.exposure != 0.0, egui::Button::new("Reset"))
                .clicked()
            {
                self.exposure = 0.0;
                changed = true;
            }
        });

        ui.add_space(6.0);

        ui.label("Saturation");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.saturation, -100.0..=100.0))
                .changed();

            if ui
                .add_enabled(self.saturation != 0.0, egui::Button::new("Reset"))
                .clicked()
            {
                self.saturation = 0.0;
                changed = true;
            }
        });

        ui.add_space(6.0);

        ui.label("Temperature (Cool / Warm)");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.temperature, -100.0..=100.0))
                .changed();

            if ui
                .add_enabled(self.temperature != 0.0, egui::Button::new("Reset"))
                .clicked()
            {
                self.temperature = 0.0;
                changed = true;
            }
        });

        ui.add_space(6.0);

        ui.label("Tint (Green / Magenta)");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::Slider::new(&mut self.tint, -100.0..=100.0))
                .changed();

            if ui
                .add_enabled(self.tint != 0.0, egui::Button::new("Reset"))
                .clicked()
            {
                self.tint = 0.0;
                changed = true;
            }
        });

        if changed {
            self.update_processed_image(ui.ctx());
        }

        ui.separator();
        ui.heading("Comparison");

        ui.checkbox(&mut self.comparison_enabled, "Enable Before / After");

        if self.comparison_enabled {
            ui.label("Divider Position");

            ui.add(egui::Slider::new(&mut self.comparison_position, 0.0..=1.0).show_value(false));

            ui.label("Drag the divider on the image.");
            ui.label("Left: Original");
            ui.label("Right: Adjusted");
        }
        if before
            != (
                self.brightness,
                self.contrast,
                self.exposure,
                self.saturation,
                self.temperature,
                self.tint,
                self.comparison_enabled,
                self.comparison_position,
            )
        {
            self.project_dirty = true;
        }
    }
    fn show_viewer(&mut self, ui: &mut egui::Ui) {
        if self.texture.is_none() {
            ui.centered_and_justified(|ui| {
                ui.heading("Open an image to begin");
            });
            return;
        }

        ui.horizontal(|ui| {
            if ui.button("Fit").clicked() {
                self.fit_pending = true;
            }

            if ui.button("100%").clicked() {
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
                self.fit_pending = false;
            }

            if ui.button("-").clicked() {
                self.zoom = (self.zoom / 1.25).max(0.001);
                self.offset = Vec2::ZERO;
                self.fit_pending = false;
            }

            if ui.button("+").clicked() {
                self.zoom = (self.zoom * 1.25).min(16.0);
                self.offset = Vec2::ZERO;
                self.fit_pending = false;
            }

            ui.separator();
            ui.label(format!("{:.1}%", self.zoom * 100.0));
        });

        ui.separator();

        let available = ui.available_size();

        if available.x <= 0.0 || available.y <= 0.0 {
            return;
        }

        if self.fit_pending {
            self.fit_image(available);
        }

        let (canvas, response) = ui.allocate_exact_size(available, Sense::hover());

        let painter = ui.painter().with_clip_rect(canvas);

        painter.rect_filled(canvas, 0.0, Color32::from_rgb(24, 24, 28));

        let center = canvas.center();

        let (
            button_down,
            button_pressed,
            double_clicked,
            pointer_delta,
            pointer_position,
            space_down,
        ) = ui.ctx().input(|i| {
            let pan_button = match self.controls.pan {
                PanButton::Middle => egui::PointerButton::Middle,
                PanButton::Left | PanButton::SpaceLeft => egui::PointerButton::Primary,
            };

            let fit_button = match self.controls.fit {
                FitButton::Left => egui::PointerButton::Primary,
                FitButton::Middle => egui::PointerButton::Middle,
            };

            (
                i.pointer.button_down(pan_button),
                i.pointer.button_pressed(pan_button),
                i.pointer.button_double_clicked(fit_button),
                i.pointer.delta(),
                i.pointer.interact_pos(),
                i.key_down(egui::Key::Space),
            )
        });

        let image_width = self.image_size[0] as f32 * self.zoom;
        let image_height = self.image_size[1] as f32 * self.zoom;
        let image_rect =
            Rect::from_center_size(center + self.offset, Vec2::new(image_width, image_height));

        let divider_x = image_rect.left() + image_rect.width() * self.comparison_position;

        let (primary_down, primary_pressed) = ui.ctx().input(|i| {
            (
                i.pointer.button_down(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Primary),
            )
        });

        if !primary_down {
            self.divider_dragging = false;
        }

        if self.comparison_enabled && primary_pressed {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                if canvas.contains(pos)
                    && image_rect.contains(pos)
                    && self.processed_texture.is_some()
                    && (pos.x - divider_x).abs() <= 12.0
                {
                    self.divider_dragging = true;
                    self.panning = false;
                }
            }
        }

        if self.divider_dragging && primary_down {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                if image_rect.width() > 0.0 {
                    self.project_dirty = true;
                    self.comparison_position =
                        ((pos.x - image_rect.left()) / image_rect.width()).clamp(0.0, 1.0);
                }
            }
        }
        let pointer_inside = pointer_position
            .map(|pos| canvas.contains(pos))
            .unwrap_or(false);

        let modifier_ok = match self.controls.pan {
            PanButton::SpaceLeft => space_down,
            _ => true,
        };

        if button_pressed && pointer_inside && modifier_ok && !self.divider_dragging {
            self.panning = true;
        }

        if !button_down {
            self.panning = false;
        }

        if double_clicked && pointer_inside {
            self.fit_image(available);
            self.panning = false;
        }

        if self.panning && button_down {
            self.offset += pointer_delta;
        }

        if response.hovered() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);

            if scroll.abs() > 0.01 {
                let factor = (scroll * 0.002 * self.controls.zoom_sensitivity).exp();

                let anchor = response.hover_pos().unwrap_or(center);

                self.zoom_at(factor, anchor, center);
            }
        }

        if let Some(original) = &self.texture {
            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));

            if let Some(processed) = &self.processed_texture {
                painter.image(processed.id(), image_rect, uv, Color32::WHITE);

                if self.comparison_enabled {
                    let split = image_rect.left() + image_rect.width() * self.comparison_position;

                    let left_clip = Rect::from_min_max(
                        canvas.min,
                        Pos2::new(split.clamp(canvas.left(), canvas.right()), canvas.bottom()),
                    );

                    painter.with_clip_rect(left_clip).image(
                        original.id(),
                        image_rect,
                        uv,
                        Color32::WHITE,
                    );

                    if split >= canvas.left()
                        && split <= canvas.right()
                        && image_rect.intersects(canvas)
                    {
                        let top = image_rect.top().max(canvas.top());
                        let bottom = image_rect.bottom().min(canvas.bottom());

                        if top < bottom {
                            painter.line_segment(
                                [Pos2::new(split, top), Pos2::new(split, bottom)],
                                egui::Stroke::new(2.0_f32, Color32::WHITE),
                            );
                        }
                    }
                }
            } else {
                painter.image(original.id(), image_rect, uv, Color32::WHITE);
            }
        }

        let over_divider = self.comparison_enabled
            && self.processed_texture.is_some()
            && pointer_position
                .map(|pos| {
                    canvas.contains(pos)
                        && image_rect.contains(pos)
                        && (pos.x
                            - (image_rect.left() + image_rect.width() * self.comparison_position))
                            .abs()
                            <= 12.0
                })
                .unwrap_or(false);

        if self.divider_dragging || over_divider {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if self.panning {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        }
    }
}

impl eframe::App for PixelForge {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Intercept the native window X button before the
        // operating system closes the application.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.project_dirty && !self.exit_allowed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.exit_confirmation = true;
            }
        }

        self.poll_export(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Image...").clicked() {
                        ui.close();
                        self.request_open(PendingOpen::Image, ctx);
                    }

                    if ui.button("Open Project...").clicked() {
                        ui.close();
                        self.request_open(PendingOpen::Project, ctx);
                    }

                    ui.separator();

                    let can_save = self.original_pixels.is_some() && !self.exporting;

                    if ui
                        .add_enabled(can_save, egui::Button::new("Save Project"))
                        .clicked()
                    {
                        ui.close();
                        self.save_project(false);
                    }

                    if ui
                        .add_enabled(can_save, egui::Button::new("Save Project As..."))
                        .clicked()
                    {
                        ui.close();
                        self.save_project(true);
                    }

                    ui.separator();

                    if ui
                        .add_enabled(can_save, egui::Button::new("Close Image / Project"))
                        .clicked()
                    {
                        ui.close();
                        self.request_open(PendingOpen::Close, ctx);
                    }

                    ui.separator();

                    if ui.button("Exit").clicked() {
                        ui.close();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    let can_edit = self.original_pixels.is_some() && !self.exporting;

                    if ui
                        .add_enabled(
                            can_edit && self.brightness != 0.0,
                            egui::Button::new("Reset Brightness"),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.brightness = 0.0;
                        self.project_dirty = true;
                        self.update_processed_image(ctx);
                    }

                    if ui
                        .add_enabled(
                            can_edit && self.contrast != 0.0,
                            egui::Button::new("Reset Contrast"),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.contrast = 0.0;
                        self.project_dirty = true;
                        self.update_processed_image(ctx);
                    }

                    if ui
                        .add_enabled(
                            can_edit
                                && (self.brightness != 0.0
                                    || self.contrast != 0.0
                                    || self.exposure != 0.0
                                    || self.saturation != 0.0
                                    || self.temperature != 0.0
                                    || self.tint != 0.0),
                            egui::Button::new("Reset All Adjustments"),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.brightness = 0.0;
                        self.contrast = 0.0;
                        self.exposure = 0.0;
                        self.saturation = 0.0;
                        self.temperature = 0.0;
                        self.tint = 0.0;
                        self.project_dirty = true;
                        self.update_processed_image(ctx);
                    }

                    ui.separator();

                    if ui.button("Settings...").clicked() {
                        ui.close();
                        self.show_settings = true;
                    }
                });

                ui.menu_button("Export", |ui| {
                    let can_export = self.original_pixels.is_some() && !self.exporting;

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as PNG..."))
                        .clicked()
                    {
                        ui.close();
                        self.start_export(ExportFormat::Png);
                    }

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as JPEG..."))
                        .clicked()
                    {
                        ui.close();
                        self.start_export(ExportFormat::Jpeg);
                    }

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as WebP..."))
                        .clicked()
                    {
                        ui.close();
                        self.start_export(ExportFormat::WebP);
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .add_enabled(
                            self.original_pixels.is_some(),
                            egui::Button::new("Fit Image to Window"),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.fit_pending = true;
                    }

                    ui.separator();

                    if ui.selectable_label(!self.advanced, "Simple Mode").clicked() {
                        self.advanced = false;
                        ui.close();
                    }

                    if ui
                        .selectable_label(self.advanced, "Advanced Mode")
                        .clicked()
                    {
                        self.advanced = true;
                        ui.close();
                    }
                });

                ui.separator();
                ui.strong("PixelForge");

                if self.project_dirty {
                    ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                }
            });
        });
        egui::SidePanel::right("tools")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                if self.texture.is_some() {
                    self.show_adjustments(ui);
                    ui.add_space(12.0);
                }

                ui.heading("Image Tools");
                ui.separator();

                ui.label("Upscaling");
                ui.add_enabled(false, egui::Button::new("2x Upscale"));
                ui.add_enabled(false, egui::Button::new("4x Upscale"));

                ui.separator();
                ui.label("Restoration");
                ui.add_enabled(false, egui::Button::new("Denoise"));
                ui.add_enabled(false, egui::Button::new("Deblur"));
                ui.add_enabled(false, egui::Button::new("Restore Faces"));

                if self.advanced {
                    ui.separator();
                    ui.heading("Advanced");
                    ui.label("Layers and masks coming soon.");
                }

                if let Some(path) = &self.image_path {
                    ui.separator();
                    ui.heading("Image Information");

                    if let Some(name) = path.file_name() {
                        ui.label(name.to_string_lossy());
                    }

                    ui.label(format!(
                        "{} x {} pixels",
                        self.image_size[0], self.image_size[1]
                    ));

                    ui.label(format!("{:.2} MB", self.file_size as f64 / 1_048_576.0));
                }
            });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 76.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(76.0);
                    if self.exporting || self.export_result.is_some() {
                        ui.horizontal(|ui| {
                            if self.exporting && self.export_encoding {
                                ui.spinner();
                                ui.label(&self.export_stage);
                            } else if self.export_result == Some(false) {
                                ui.colored_label(egui::Color32::LIGHT_RED, "Export failed");
                            } else {
                                ui.label(&self.export_stage);
                                ui.label(format!("{:.0}%", self.export_progress * 100.0));
                            }
                        });

                        if self.export_result != Some(false) {
                            ui.add(
                                egui::ProgressBar::new(self.export_progress)
                                    .desired_width(ui.available_width()),
                            );
                        }

                        if self.export_result == Some(true) {
                            ui.label("Image saved successfully");
                        }
                    }
                },
            );
            ui.horizontal(|ui| {
                ui.label(&self.status);

                if self.texture.is_some() {
                    ui.separator();

                    ui.label(format!("{} x {}", self.image_size[0], self.image_size[1]));

                    ui.separator();

                    ui.label(format!("Zoom: {:.1}%", self.zoom * 100.0));
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_viewer(ui);
        });

        self.show_controls_settings(ctx);
        self.show_unsaved_prompt(ctx);
        self.show_exit_confirmation(ctx);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PixelForge v0.2.1")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "PixelForge",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(PixelForge::default()))
        }),
    )
}
