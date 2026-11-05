//! Native fullscreen with a separate normal-window snapshot (physical pixels).
use egui::{Context, ViewportCommand as Cmd};

#[derive(Default)]
pub struct Display {
    fullscreen: bool,
    applied: Option<bool>,
    placement: Option<[f32; 4]>,
    last_position: Option<[f32; 2]>,
    startup_prepared: bool,
    startup_frames: u8,
    last_native_scale: Option<f32>,
    scale_settle_frames: u8,
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
            state.last_position = serde_json::from_value(value["last_position"].clone()).ok()
                .filter(|p: &[f32; 2]| p.iter().all(|v| v.is_finite()));
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
                    "fullscreen": self.fullscreen, "placement": self.placement, "maximized": self.maximized, "last_position": self.last_position
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
        crate::interaction::label(ui,"The fullscreen shortcut can be changed under Hotkeys.");
        crate::interaction::label(ui,"Uses the entire current monitor, including ultrawide displays.");
        if ui.button("Reset Window Size and Position").on_hover_text("Return to a movable 1200 × 800 window on the current display.").clicked() {
            self.reset_window(ui.ctx());
        }
        if !self.error.is_empty() {
            ui.colored_label(egui::Color32::YELLOW, &self.error);
        }
    }
    fn reset_window(&mut self, ctx: &Context) {
        let scale = ctx.pixels_per_point();
        let origin = if self.fullscreen {
            ctx.input(|i| i.viewport().outer_rect.map(|r| r.min * scale)).unwrap_or(egui::Pos2::ZERO)
        } else { egui::Pos2::ZERO };
        self.placement = Some([origin.x + 80.0, origin.y + 80.0, 1200.0, 800.0]);
        self.last_position = Some([origin.x + 80.0, origin.y + 80.0]);
        self.fullscreen = false;
        self.maximized = false;
        self.restore_frames = 4;
        ctx.send_viewport_cmd(Cmd::Fullscreen(false));
        ctx.request_repaint();
        self.save();
    }
    /// Fullscreen has no OS title bar, so keep its window actions in the app menu bar.
    pub fn window_controls(&mut self, ui: &mut egui::Ui) {
        // The parent lays these out right-to-left, matching Windows caption order.
        if caption_button(ui, Caption::Close).clicked() {
            // Use the native close-request path, including existing unsaved-work checks.
            ui.ctx().send_viewport_cmd(Cmd::Close);
        }
        if caption_button(ui, Caption::Restore).clicked() {
            self.maximize_or_restore(ui.ctx());
        }
        if caption_button(ui, Caption::Minimize).clicked() {
            ui.ctx().send_viewport_cmd(Cmd::Minimized(true));
        }
    }
    fn maximize_or_restore(&mut self, ctx: &Context) {
        if self.fullscreen { self.fullscreen = false; }
        else { ctx.send_viewport_cmd(Cmd::Maximized(!ctx.input(|i| i.viewport().maximized.unwrap_or(false)))); }
        ctx.request_repaint();
    }
    pub fn title_drag(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let response = ui.interact(rect, ui.id().with("window_title_drag"), egui::Sense::click_and_drag());
        if !self.fullscreen {
            if response.double_clicked() { self.maximize_or_restore(ui.ctx()); }
            else if response.drag_started_by(egui::PointerButton::Primary) { ui.ctx().send_viewport_cmd(Cmd::StartDrag); }
        }
    }
    pub fn resize_edges(&self, ctx: &Context) {
        if self.fullscreen || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) { return; }
        let r = ctx.screen_rect();
        if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
            // Native moves can deliver positions outside the client area, especially
            // across different monitor scales. They are never resize hit targets.
            if !r.contains(p) || ctx.input(|i| i.viewport().focused == Some(false)) { return; }
            let left=p.x < r.left()+5.0; let right=p.x > r.right()-5.0;
            let top=p.y < r.top()+5.0; let bottom=p.y > r.bottom()-5.0;
            use egui::ResizeDirection::*;
            let direction=match (left,right,top,bottom) {
                (true,_,true,_)=>Some(NorthWest), (_,true,true,_)=>Some(NorthEast),
                (true,_,_,true)=>Some(SouthWest), (_,true,_,true)=>Some(SouthEast),
                (true,_,_,_)=>Some(West), (_,true,_,_)=>Some(East),
                (_,_,true,_)=>Some(North), (_,_,_,true)=>Some(South), _=>None,
            };
            if let Some(direction)=direction {
                ctx.set_cursor_icon(match direction { North|South=>egui::CursorIcon::ResizeVertical, East|West=>egui::CursorIcon::ResizeHorizontal, NorthWest|SouthEast=>egui::CursorIcon::ResizeNwSe, _=>egui::CursorIcon::ResizeNeSw });
                let pressed_on_edge = ctx.input(|i| i.events.iter().any(|event| matches!(event,
                    egui::Event::PointerButton {pos, button: egui::PointerButton::Primary, pressed: true, ..}
                    if r.contains(*pos) && (pos.x < r.left()+5.0 || pos.x > r.right()-5.0 || pos.y < r.top()+5.0 || pos.y > r.bottom()-5.0)
                )));
                if pressed_on_edge {ctx.send_viewport_cmd(Cmd::BeginResize(direction));}
            }
        }
    }
    pub fn toggle(&mut self) {self.fullscreen=!self.fullscreen;}
    pub fn update(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            self.fullscreen = !self.fullscreen;
        }
        // Position a normal window on the saved display before asking Windows to
        // enter fullscreen/maximized mode, which always uses the current monitor.
        if self.applied.is_none() && !self.startup_prepared {
            self.startup_prepared = true;
            self.startup_frames = 3;
            let scale = ctx.pixels_per_point();
            ctx.send_viewport_cmd(Cmd::Fullscreen(false));
            ctx.send_viewport_cmd(Cmd::Decorations(false));
            ctx.send_viewport_cmd(Cmd::Maximized(false));
            if let Some([_, _, w, h]) = self.placement {
                ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(w / scale, h / scale)));
            }
            if let Some([x,y]) = self.last_position.or(self.placement.map(|p| [p[0],p[1]])) {
                ctx.send_viewport_cmd(Cmd::OuterPosition(egui::pos2(x / scale,y / scale)));
            }
            ctx.request_repaint();
            return;
        }
        if self.startup_frames > 0 {
            self.startup_frames -= 1;
            ctx.request_repaint();
            if self.startup_frames > 0 { return; }
        }
        let viewport = ctx.input(|i| i.viewport().clone());
        if let Some(native_scale) = viewport.native_pixels_per_point {
            let changed = self.last_native_scale.replace(native_scale).is_some_and(|old| (old-native_scale).abs()>0.001);
            if changed && self.applied == Some(false) && !self.fullscreen && viewport.maximized != Some(true) && self.restore_frames == 0 {
                if let Some([_,_,w,h]) = self.placement {
                    // Keep the previous physical footprint when Windows changes DPI.
                    // Do not persist the intermediate geometry from that transition.
                    let scale = ctx.pixels_per_point();
                    let size = fit_window_size(egui::vec2(w,h), viewport.monitor_size.map(|m|m*scale));
                    ctx.send_viewport_cmd(Cmd::InnerSize(size / scale));
                    self.scale_settle_frames = 4;
                    ctx.request_repaint();
                    return;
                }
            }
        }
        if self.scale_settle_frames > 0 {
            self.scale_settle_frames -= 1;
            ctx.request_repaint();
            return;
        }
        if self.applied == Some(self.fullscreen) && self.restore_frames == 0 {
            let viewport = ctx.input(|i| i.viewport().clone());
            if viewport.minimized != Some(true) && viewport.fullscreen == Some(self.fullscreen) {
                if let Some(rect) = viewport.outer_rect {
                    let p = [rect.min.x * ctx.pixels_per_point(), rect.min.y * ctx.pixels_per_point()];
                    if p.iter().all(|v| v.is_finite()) && self.last_position != Some(p) {
                        self.last_position = Some(p);
                        self.save();
                    }
                }
            }
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
            let startup = self.applied.is_none();
            ctx.send_viewport_cmd(Cmd::Fullscreen(self.fullscreen));
            if startup && !self.fullscreen {ctx.send_viewport_cmd(Cmd::Maximized(self.maximized));}
            if !self.fullscreen {
                ctx.send_viewport_cmd(Cmd::Decorations(false));
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
fn fit_window_size(size: egui::Vec2, monitor: Option<egui::Vec2>) -> egui::Vec2 {
    if let Some(monitor) = monitor.filter(|m| m.is_finite() && m.x > 0.0 && m.y > 0.0) {
        size.min(monitor * 0.9)
    } else { size }
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
        Caption::Restore => "Maximize / restore window",
        Caption::Close => "Close PixelForge",
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 22.0), egui::Sense::click());
    let response = response.on_hover_text(label);
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, 3.0, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
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
    fn dpi_change_preserves_saved_size_instead_of_expanded_intermediate_geometry() {
        let ctx = Context::default();
        let mut display = Display {
            applied: Some(false), last_native_scale: Some(1.5),
            placement: Some([80.0,80.0,1200.0,800.0]), ..Default::default()
        };
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.fullscreen = Some(false);
        viewport.monitor_size = Some(egui::vec2(1920.0,1080.0));
        viewport.inner_rect = Some(egui::Rect::from_min_size(egui::pos2(100.0,100.0),egui::vec2(4502.0,1539.0)));
        viewport.outer_rect = viewport.inner_rect;
        let output = ctx.run(input, |ctx| display.update(ctx));
        assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.contains(&Cmd::InnerSize(egui::vec2(1200.0,800.0))));
        assert_eq!(display.placement, Some([80.0,80.0,1200.0,800.0]));
        assert_eq!(fit_window_size(egui::vec2(4502.0,1539.0),Some(egui::vec2(1920.0,1080.0))),egui::vec2(1728.0,972.0));
    }
    #[test]
    fn pointer_outside_window_cannot_start_native_resize() {
        let ctx = Context::default();
        let display = Display::default();
        let pos = egui::pos2(1500.0, 900.0);
        let output = ctx.run(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0,800.0))),
            events: vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default(),
            }], ..Default::default()
        }, |ctx| display.resize_edges(ctx));
        assert!(!output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c| matches!(c, Cmd::BeginResize(_))));
    }
    #[test]
    fn recovery_replaces_stuck_geometry_with_normal_window() {
        let ctx = Context::default();
        let mut display = Display {fullscreen:true, placement:Some([370.0,771.0,4502.0,1539.0]), applied:Some(true), ..Default::default()};
        let _ = ctx.run(Default::default(), |ctx| display.reset_window(ctx));
        for _ in 0..4 {let _ = ctx.run(Default::default(), |ctx| display.update(ctx));}
        assert!(!display.fullscreen);
        assert_eq!(display.placement,Some([80.0,80.0,1200.0,800.0]));
    }
    #[test]
    fn startup_positions_on_saved_display_before_fullscreen() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.5);
        let mut display = Display {
            fullscreen: true,
            last_position: Some([-3840.0, 0.0]),
            placement: Some([120.0, 60.0, 1500.0, 900.0]),
            ..Default::default()
        };
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&Cmd::OuterPosition(egui::pos2(-2560.0, 0.0))));
        assert!(commands.contains(&Cmd::InnerSize(egui::vec2(1000.0, 600.0))));
        assert!(!commands.contains(&Cmd::Fullscreen(true)));
        for _ in 0..2 {let _ = ctx.run(Default::default(), |ctx| display.update(ctx));}
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.contains(&Cmd::Fullscreen(true)));
        assert_eq!(display.placement, Some([120.0, 60.0, 1500.0, 900.0]));
    }
    #[test]
    fn startup_restores_windowed_size_position_and_maximized_state() {
        let ctx = Context::default();
        let mut display = Display {
            maximized: true,
            placement: Some([-1800.0, 80.0, 1200.0, 800.0]),
            ..Default::default()
        };
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&Cmd::OuterPosition(egui::pos2(-1800.0, 80.0))));
        assert!(commands.contains(&Cmd::InnerSize(egui::vec2(1200.0, 800.0))));
        for _ in 0..2 {let _ = ctx.run(Default::default(), |ctx| display.update(ctx));}
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.contains(&Cmd::Maximized(true)));
    }
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
