//! Native fullscreen with a separate normal-window snapshot (physical pixels).
use egui::{Context, ViewportCommand as Cmd};

#[derive(Default)]
pub struct Display {
    fullscreen: bool,
    applied: Option<bool>,
    placement: Option<[f32; 4]>,
    maximized: bool,
    restore_frames: u8,
    error: String,
}
impl Display {
    pub fn load() -> Self {
        let mut state = Self::default();
        if let Some(value) = crate::profile::settings_root()
            .and_then(|p| std::fs::read(p.join("display.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            state.fullscreen = value["fullscreen"].as_bool().unwrap_or(false);
            state.maximized = value["maximized"].as_bool().unwrap_or(false);
            state.placement = serde_json::from_value(value["placement"].clone())
                .ok()
                .filter(|v: &[f32; 4]| valid_placement(*v));
        }
        state
    }
    #[cfg(test)]
    fn save(&mut self) {}
    #[cfg(not(test))]
    fn save(&mut self) {
        let result = (|| -> std::io::Result<()> {
            let root = crate::profile::settings_root()
                .ok_or_else(|| std::io::Error::other("APPDATA unavailable"))?;
            std::fs::create_dir_all(&root)?;
            let path = root.join("display.json");
            let temp = root.join("display.json.new");
            std::fs::write(
                &temp,
                serde_json::to_vec(&serde_json::json!({
                    "fullscreen": self.fullscreen, "placement": self.placement, "maximized": self.maximized
                }))?,
            )?;
            std::fs::rename(temp, path)
        })();
        self.error = result
            .err()
            .map(|e| format!("Cannot save display settings: {e}"))
            .unwrap_or_default();
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Display");
        ui.separator();
        ui.radio_value(&mut self.fullscreen, false, "Windowed");
        ui.radio_value(
            &mut self.fullscreen,
            true,
            "Fullscreen (hide title bar and taskbar)",
        );
        ui.label("F11 toggles fullscreen.");
        ui.label("Uses the entire current monitor, including ultrawide displays.");
        if !self.error.is_empty() {
            ui.colored_label(egui::Color32::YELLOW, &self.error);
        }
    }
    /// Fullscreen has no OS title bar, so keep its window actions in the app menu bar.
    pub fn window_controls(&mut self, ui: &mut egui::Ui) {
        if !self.fullscreen {
            return;
        }
        // The parent lays these out right-to-left, matching Windows caption order.
        if caption_button(ui, Caption::Close).clicked() {
            // Use the native close-request path, including existing unsaved-work checks.
            ui.ctx().send_viewport_cmd(Cmd::Close);
        }
        if caption_button(ui, Caption::Restore).clicked() {
            self.fullscreen = false;
            ui.ctx().request_repaint();
        }
        if caption_button(ui, Caption::Minimize).clicked() {
            ui.ctx().send_viewport_cmd(Cmd::Minimized(true));
        }
    }
    pub fn update(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            self.fullscreen = !self.fullscreen;
        }
        // Observe placement only after transitions have settled. Never persist a fullscreen rectangle.
        if self.applied == Some(false) && self.restore_frames == 0 {
            let viewport = ctx.input(|i| i.viewport().clone());
            if viewport.fullscreen == Some(false) && viewport.minimized != Some(true) {
                let maximized = viewport.maximized.unwrap_or(false);
                if self.maximized != maximized {
                    self.maximized = maximized;
                    self.save();
                }
                if !self.maximized {
                    if let (Some(outer), Some(inner)) = (viewport.outer_rect, viewport.inner_rect) {
                        let scale = ctx.pixels_per_point();
                        let placement = [
                            outer.min.x * scale,
                            outer.min.y * scale,
                            inner.width() * scale,
                            inner.height() * scale,
                        ];
                        if valid_placement(placement) && self.placement != Some(placement) {
                            self.placement = Some(placement);
                            self.save();
                        }
                    }
                }
            }
        }
        if self.applied != Some(self.fullscreen) {
            // On startup eframe restores native placement and mode; override only mode.
            ctx.send_viewport_cmd(Cmd::Fullscreen(self.fullscreen));
            if !self.fullscreen {
                ctx.send_viewport_cmd(Cmd::Decorations(true));
                if self.applied.is_some() || ctx.input(|i| i.viewport().fullscreen == Some(true)) {
                    self.restore_frames = 3;
                }
            }
            self.applied = Some(self.fullscreen);
            self.save();
        }
        if self.restore_frames > 0 {
            self.restore_frames -= 1;
            if self.restore_frames == 0 {
                if let Some([x, y, w, h]) = self.placement {
                    let scale = ctx.pixels_per_point();
                    ctx.send_viewport_cmd(Cmd::Maximized(false));
                    ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(w / scale, h / scale)));
                    ctx.send_viewport_cmd(Cmd::OuterPosition(egui::pos2(x / scale, y / scale)));
                }
                ctx.send_viewport_cmd(Cmd::Maximized(self.maximized));
            }
            ctx.request_repaint();
        }
    }
}
#[derive(Clone, Copy)]
enum Caption {
    Minimize,
    Restore,
    Close,
}
fn caption_button(ui: &mut egui::Ui, caption: Caption) -> egui::Response {
    let label = match caption {
        Caption::Minimize => "Minimize",
        Caption::Restore => "Restore window (leave fullscreen)",
        Caption::Close => "Close PixelForge",
    };
    let response = ui
        .add(egui::Button::new("").min_size(egui::vec2(40.0, 28.0)))
        .on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let painter = ui.painter();
    let center = response.rect.center();
    if matches!(caption, Caption::Close) && response.hovered() {
        painter.rect_filled(response.rect, 2.0, egui::Color32::from_rgb(190, 35, 45));
    }
    let stroke = egui::Stroke::new(1.5_f32, ui.visuals().text_color());
    match caption {
        Caption::Minimize => {
            painter.line_segment(
                [
                    center + egui::vec2(-5.0, 2.0),
                    center + egui::vec2(5.0, 2.0),
                ],
                stroke,
            );
        }
        Caption::Restore => {
            for offset in [egui::vec2(2.0, -2.0), egui::Vec2::ZERO] {
                painter.rect_stroke(
                    egui::Rect::from_center_size(center + offset, egui::vec2(9.0, 9.0)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Caption::Close => {
            painter.line_segment(
                [
                    center + egui::vec2(-5.0, -5.0),
                    center + egui::vec2(5.0, 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-5.0, 5.0),
                    center + egui::vec2(5.0, -5.0),
                ],
                stroke,
            );
        }
    }
    response
}
fn valid_placement(v: [f32; 4]) -> bool {
    v.iter().all(|n| n.is_finite()) && v[2] >= 100.0 && v[3] >= 100.0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_accepts_ultrawide_and_negative_monitor_coordinates() {
        assert!(valid_placement([-5120.0, 0.0, 5120.0, 1440.0]));
        assert!(!valid_placement([0.0, 0.0, f32::NAN, 900.0]));
    }
    #[test]
    fn restores_physical_ultrawide_geometry_at_150_percent_scale() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.5);
        let mut state = Display {
            applied: Some(false),
            restore_frames: 1,
            placement: Some([-5120.0, 120.0, 4800.0, 1200.0]),
            ..Default::default()
        };
        let output = ctx.run(Default::default(), |ctx| state.update(ctx));
        assert_eq!(ctx.pixels_per_point(), 1.5);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&Cmd::InnerSize(egui::vec2(3200.0, 800.0))));
        assert!(commands.contains(&Cmd::OuterPosition(egui::pos2(-5120.0 / 1.5, 80.0))));
    }
    #[test]
    fn only_f11_toggles_fullscreen_without_changing_geometry() {
        let ctx = Context::default();
        let mut display = Display {
            applied: Some(false),
            placement: Some([100.0, 80.0, 1400.0, 900.0]),
            ..Default::default()
        };
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::F11,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
        let output = ctx.run(input, |ctx| display.update(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&Cmd::Fullscreen(true))
        );
        assert!(display.fullscreen);
        assert_eq!(display.placement, Some([100.0, 80.0, 1400.0, 900.0]));
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
        let output = ctx.run(input, |ctx| display.update(ctx));
        assert!(display.fullscreen, "Escape must leave fullscreen unchanged");
        assert!(
            !output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&Cmd::Fullscreen(false))
        );
        let mut input = egui::RawInput::default();
        for pressed in [false, true] {
            input.events.push(egui::Event::Key {
                key: egui::Key::F11,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            });
        }
        let output = ctx.run(input, |ctx| display.update(ctx));
        assert!(!display.fullscreen);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&Cmd::Fullscreen(false))
        );
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| display.update(ctx));
        }
        assert_eq!(display.placement, Some([100.0, 80.0, 1400.0, 900.0]));
    }
}
