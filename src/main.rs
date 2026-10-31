use eframe::egui;
use egui::{
    Color32, ColorImage, Pos2, Rect, Sense,
    TextureHandle, TextureOptions, Vec2,
};
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
        Some(
            PathBuf::from(base)
                .join("PixelForge")
                .join("controls.cfg"),
        )
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
                            settings.zoom_sensitivity =
                                value.clamp(0.25, 3.0);
                        }
                    }
                }
                _ => {}
            }
        }

        settings
    }

    fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path().ok_or_else(|| {
            std::io::Error::other("APPDATA is unavailable")
        })?;

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

struct PixelForge {
    texture: Option<TextureHandle>,
    processed_texture: Option<TextureHandle>,
    original_pixels: Option<image::RgbaImage>,
    brightness: f32,
    contrast: f32,
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
}

impl Default for PixelForge {
    fn default() -> Self {
        Self {
            texture: None,
            processed_texture: None,
            original_pixels: None,
            brightness: 0.0,
            contrast: 0.0,
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
        }
    }
}

impl PixelForge {
    fn open_image(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff"],
            )
            .pick_file()
        else {
            return;
        };

        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let size = [
                    rgba.width() as usize,
                    rgba.height() as usize,
                ];

                let color_image =
                    ColorImage::from_rgba_unmultiplied(
                        size,
                        rgba.as_raw(),
                    );

                self.texture = Some(ctx.load_texture(
                    "current_image",
                    color_image,
                    TextureOptions::LINEAR,
                ));

                self.file_size = std::fs::metadata(&path)
                    .map(|m| m.len())
                    .unwrap_or(0);

                self.original_pixels = Some(rgba);
                self.processed_texture = None;
                self.brightness = 0.0;
                self.contrast = 0.0;
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

    fn fit_image(&mut self, available: Vec2) {
        let w = self.image_size[0] as f32;
        let h = self.image_size[1] as f32;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        self.zoom = (available.x / w)
            .min(available.y / h)
            .clamp(0.001, 16.0);

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

        self.offset =
            anchor_offset - (anchor_offset - self.offset) * ratio;

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
                    .radio_value(
                        &mut self.controls.pan,
                        PanButton::Left,
                        "Left-click + drag",
                    )
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
                        egui::Slider::new(
                            &mut self.controls.zoom_sensitivity,
                            0.25..=3.0,
                        )
                        .custom_formatter(|value, _| {
                            format!("{:.0}%", value * 100.0)
                        }),
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

        let brightness = self.brightness * 2.55;
        let contrast = 1.0 + self.contrast / 100.0;

        for pixel in adjusted.pixels_mut() {
            for channel in 0..3 {
                let value = pixel[channel] as f32;
                let result =
                    (value - 127.5) * contrast + 127.5 + brightness;

                pixel[channel] = result.clamp(0.0, 255.0) as u8;
            }
        }

        let size = [
            adjusted.width() as usize,
            adjusted.height() as usize,
        ];

        let color_image =
            ColorImage::from_rgba_unmultiplied(
                size,
                adjusted.as_raw(),
            );

        if let Some(texture) = &mut self.processed_texture {
            texture.set(color_image, TextureOptions::LINEAR);
        } else {
            self.processed_texture = Some(ctx.load_texture(
                "processed_image",
                color_image,
                TextureOptions::LINEAR,
            ));
        }

        self.status = "Preview updated".into();
    }

    fn show_adjustments(&mut self, ui: &mut egui::Ui) {
        ui.heading("Image Adjustments");
        ui.separator();

        let mut changed = false;

        ui.label("Brightness");
        changed |= ui.add(
            egui::Slider::new(
                &mut self.brightness,
                -100.0..=100.0,
            )
        ).changed();

        ui.label("Contrast");
        changed |= ui.add(
            egui::Slider::new(
                &mut self.contrast,
                -100.0..=100.0,
            )
        ).changed();

        if ui.button("Reset Adjustments").clicked() {
            self.brightness = 0.0;
            self.contrast = 0.0;
            changed = true;
        }

        if changed {
            self.update_processed_image(ui.ctx());
        }

        ui.separator();
        ui.heading("Comparison");

        ui.checkbox(
            &mut self.comparison_enabled,
            "Enable Before / After",
        );

        if self.comparison_enabled {
            ui.label("Divider Position");
            ui.add(
                egui::Slider::new(
                    &mut self.comparison_position,
                    0.0..=1.0,
                )
                .show_value(false),
            );

            ui.label("Drag the divider on the image.");
            ui.label("Left: Original");
            ui.label("Right: Adjusted");
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

        let (canvas, response) =
            ui.allocate_exact_size(available, Sense::hover());

        let painter = ui.painter().with_clip_rect(canvas);

        painter.rect_filled(
            canvas,
            0.0,
            Color32::from_rgb(24, 24, 28),
        );

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
                PanButton::Left | PanButton::SpaceLeft => {
                    egui::PointerButton::Primary
                }
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
        let image_rect = Rect::from_center_size(
            center + self.offset,
            Vec2::new(image_width, image_height),
        );

        let divider_x = image_rect.left()
            + image_rect.width() * self.comparison_position;

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
                    self.comparison_position = (
                        (pos.x - image_rect.left()) / image_rect.width()
                    ).clamp(0.0, 1.0);
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
            let scroll =
                ui.ctx().input(|i| i.smooth_scroll_delta.y);

            if scroll.abs() > 0.01 {
                let factor = (
                    scroll
                        * 0.002
                        * self.controls.zoom_sensitivity
                ).exp();

                let anchor = response
                    .hover_pos()
                    .unwrap_or(center);

                self.zoom_at(factor, anchor, center);
            }
        }

        if let Some(original) = &self.texture {
            let uv = Rect::from_min_max(
                Pos2::new(0.0, 0.0),
                Pos2::new(1.0, 1.0),
            );

            if let Some(processed) = &self.processed_texture {
                painter.image(
                    processed.id(),
                    image_rect,
                    uv,
                    Color32::WHITE,
                );

                if self.comparison_enabled {
                    let split = image_rect.left()
                        + image_rect.width() * self.comparison_position;

                    let left_clip = Rect::from_min_max(
                        canvas.min,
                        Pos2::new(
                            split.clamp(canvas.left(), canvas.right()),
                            canvas.bottom(),
                        ),
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
                                [
                                    Pos2::new(split, top),
                                    Pos2::new(split, bottom),
                                ],
                                egui::Stroke::new(
                                    2.0,
                                    Color32::WHITE,
                                ),
                            );
                        }
                    }
                }
            } else {
                painter.image(
                    original.id(),
                    image_rect,
                    uv,
                    Color32::WHITE,
                );
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(
                if self.panning {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Default
                },
            );
        }
    }
}

impl eframe::App for PixelForge {
    fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("PixelForge");
                ui.separator();

                if ui.button("Open Image").clicked() {
                    self.open_image(ctx);
                }

                if ui.button("Settings").clicked() {
                    self.show_settings = !self.show_settings;
                }

                ui.separator();

                ui.selectable_value(
                    &mut self.advanced,
                    false,
                    "Simple",
                );

                ui.selectable_value(
                    &mut self.advanced,
                    true,
                    "Advanced",
                );
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
                ui.add_enabled(
                    false,
                    egui::Button::new("Restore Faces"),
                );

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
                        self.image_size[0],
                        self.image_size[1]
                    ));

                    ui.label(format!(
                        "{:.2} MB",
                        self.file_size as f64 / 1_048_576.0
                    ));
                }
            });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);

                if self.texture.is_some() {
                    ui.separator();

                    ui.label(format!(
                        "{} x {}",
                        self.image_size[0],
                        self.image_size[1]
                    ));

                    ui.separator();

                    ui.label(format!(
                        "Zoom: {:.1}%",
                        self.zoom * 100.0
                    ));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_viewer(ui);
        });

        self.show_controls_settings(ctx);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PixelForge v0.1.5")
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


