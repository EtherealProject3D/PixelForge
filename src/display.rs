//! Native fullscreen with a separate normal-window snapshot (physical pixels).
use egui::{Context, ViewportCommand as Cmd};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Display {
    fullscreen: bool,
    image_fullscreen: bool,
    image_fullscreen_exit: Option<Instant>,
    applied: Option<bool>,
    placement: Option<[f32; 4]>,
    last_position: Option<[f32; 2]>,
    startup_prepared: bool,
    startup_frames: u8,
    last_native_scale: Option<f32>,
    dragging: Option<WindowDrag>,
    scale_settle_frames: u8,
    maximized: bool,
    restore_frames: u8,
    error: String,
}
#[derive(Clone, Copy)]
struct WindowDrag {
    pointer: egui::Vec2,
    origin: egui::Vec2,
    size: egui::Vec2,
}
impl WindowDrag {
    fn position(self, pointer: egui::Vec2) -> egui::Vec2 { self.origin + pointer - self.pointer }
}
#[cfg(windows)]
fn desktop_pointer() -> Option<(egui::Vec2, bool)> {
    use windows::Win32::{Foundation::POINT, UI::{WindowsAndMessaging::GetCursorPos, Input::KeyboardAndMouse::GetAsyncKeyState}};
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).ok()?;
        Some((egui::vec2(point.x as f32, point.y as f32), GetAsyncKeyState(1) < 0))
    }
}
#[cfg(not(windows))]
fn desktop_pointer() -> Option<(egui::Vec2, bool)> { None }
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
            else if response.drag_started_by(egui::PointerButton::Primary) {
                if let Some((pointer,_)) = desktop_pointer() {
                    let viewport = ui.input(|i| i.viewport().clone());
                    if viewport.maximized == Some(true) {
                        self.maximize_or_restore(ui.ctx());
                    } else if let (Some(outer),Some(inner)) = (viewport.outer_rect,viewport.inner_rect) {
                        let scale = ui.ctx().pixels_per_point();
                        let pointer_start = ui.input(|i| i.pointer.press_origin()).map(|p| outer.min.to_vec2()*scale + p.to_vec2()*scale).unwrap_or(pointer);
                        self.dragging = Some(WindowDrag {pointer:pointer_start, origin:outer.min.to_vec2()*scale, size:inner.size()*scale});
                        ui.ctx().request_repaint();
                    }
                }
                #[cfg(not(windows))]
                ui.ctx().send_viewport_cmd(Cmd::StartDrag);
            }
        }
    }
    pub fn resize_edges(&self, ctx: &Context) {
        if self.fullscreen || self.shows_image_view() || self.dragging.is_some() || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) { return; }
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
    pub fn toggle(&mut self) {
        if !self.shows_image_view() {
            self.fullscreen = !self.fullscreen;
        }
    }
    pub fn image_fullscreen(&self) -> bool {
        self.image_fullscreen
    }
    pub fn shows_image_view(&self) -> bool {
        self.image_fullscreen || self.image_fullscreen_exit.is_some()
    }

    pub fn set_image_fullscreen(&mut self, ctx: &Context, open: bool) {
        self.image_fullscreen_exit = (!open && self.image_fullscreen && !self.fullscreen)
            .then(|| Instant::now() + Duration::from_millis(350));
        self.image_fullscreen = open;
        ctx.request_repaint();
    }
    pub fn update(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            self.toggle();
        }
        let fullscreen = self.fullscreen || self.image_fullscreen;
        if let Some(drag) = self.dragging {
            if !fullscreen && let Some((pointer,held)) = desktop_pointer() {
                let scale = ctx.pixels_per_point();
                let position = drag.position(pointer) / scale;
                ctx.send_viewport_cmd(Cmd::OuterPosition(egui::pos2(position.x,position.y)));
                // Avoid Windows' native move/DPI size loop. Keep physical dimensions
                // stable using the original drag snapshot rather than intermediate frames.
                let current_size = ctx.input(|i| i.viewport().inner_rect.map(|r|r.size()*scale));
                if current_size.is_some_and(|size| (size-drag.size).length() > 2.0) {
                    ctx.send_viewport_cmd(Cmd::InnerSize(drag.size/scale));
                }
                if !held { self.dragging = None; self.scale_settle_frames = 4; }
                ctx.request_repaint();
                return;
            }
            self.dragging = None;
            self.scale_settle_frames = 4;
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
            if changed { self.scale_settle_frames = 4; }

        }
        if self.scale_settle_frames > 0 {
            self.scale_settle_frames -= 1;
            ctx.request_repaint();
            return;
        }
        // Keep painting the image until the native window returns to its editor geometry.
        if let Some(deadline) = self.image_fullscreen_exit {
            let scale = ctx.pixels_per_point();
            let size_restored = self.maximized || self.placement.is_none_or(|[_, _, w, h]| {
                viewport.inner_rect.is_none_or(|rect| {
                    (rect.size() * scale - egui::vec2(w, h)).length() < 4.0
                })
            });
            if self.applied == Some(false) && self.restore_frames == 0
                && viewport.fullscreen != Some(true)
                && viewport.maximized.is_none_or(|maximized| maximized == self.maximized)
                && size_restored
            {
                self.image_fullscreen_exit = None;
            } else if Instant::now() >= deadline && self.applied == Some(false) && self.restore_frames == 0 {
                self.image_fullscreen_exit = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
        if !self.shows_image_view() && self.applied == Some(fullscreen) && self.restore_frames == 0 {
            let viewport = ctx.input(|i| i.viewport().clone());
            if viewport.minimized != Some(true) && viewport.fullscreen == Some(fullscreen) {
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
        if !self.shows_image_view() && self.applied == Some(false) && self.restore_frames == 0 {
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
        if self.applied != Some(fullscreen) {
            let startup = self.applied.is_none();
            if fullscreen {
                // A pending windowed restore must not resize or maximize fullscreen.
                self.restore_frames = 0;
                // Winit retains WS_MAXIMIZE when applying the fullscreen style.
                // Clear it first so Windows can size the client area to the monitor.
                // Keep self.maximized as the windowed state to restore on exit.
                #[cfg(windows)]
                if viewport.maximized == Some(true) {
                    ctx.send_viewport_cmd(Cmd::Maximized(false));
                }
            }
            ctx.send_viewport_cmd(Cmd::Fullscreen(fullscreen));
            if startup && !fullscreen {ctx.send_viewport_cmd(Cmd::Maximized(self.maximized));}
            if !fullscreen {
                ctx.send_viewport_cmd(Cmd::Decorations(false));
                if self.applied.is_some() || ctx.input(|i| i.viewport().fullscreen == Some(true)) {
                    self.restore_frames = 3;
                }
            }
            self.applied = Some(fullscreen);
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
    fn dpi_change_does_not_issue_competing_resize_commands_or_save_intermediate_geometry() {
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
        assert!(!output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,Cmd::InnerSize(_))));
        assert_eq!(display.placement, Some([80.0,80.0,1200.0,800.0]));
        let drag = WindowDrag { pointer:egui::vec2(500.0,200.0), origin:egui::vec2(100.0,100.0), size:egui::vec2(1200.0,800.0) };
        assert_eq!(drag.position(egui::vec2(-1000.0,300.0)),egui::vec2(-1400.0,200.0));
        assert_eq!(drag.size,egui::vec2(1200.0,800.0));
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
    #[cfg(windows)]
    fn fullscreen_clears_native_maximize_and_restores_it_on_exit() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.5);
        let placement = Some([-5100.0, 120.0, 4800.0, 1200.0]);
        let mut display = Display {
            applied: Some(false),
            placement,
            ..Default::default()
        };
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.fullscreen = Some(false);
        viewport.maximized = Some(true);
        display.toggle();
        let output = ctx.run(input, |ctx| display.update(ctx));
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].commands,
            [Cmd::Maximized(false), Cmd::Fullscreen(true)],
        );
        assert!(display.maximized);
        assert_eq!(display.placement, placement);

        display.toggle();
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&Cmd::Fullscreen(false)));
        let _ = ctx.run(Default::default(), |ctx| display.update(ctx));
        let output = ctx.run(Default::default(), |ctx| display.update(ctx));
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].commands,
            [
                Cmd::Maximized(false),
                Cmd::InnerSize(egui::vec2(3200.0, 800.0)),
                Cmd::OuterPosition(egui::pos2(-3400.0, 80.0)),
                Cmd::Maximized(true),
            ],
        );
    }

    #[test]
    fn temporary_image_fullscreen_restores_the_preexisting_window_mode() {
        let ctx = Context::default();
        for fullscreen in [false, true] {
            let mut display = Display {
                fullscreen,
                applied: Some(fullscreen),
                ..Default::default()
            };
            let entered = ctx.run(Default::default(), |ctx| {
                display.set_image_fullscreen(ctx, true);
                display.update(ctx);
            });
            assert_eq!(display.applied, Some(true));
            assert_eq!(display.fullscreen, fullscreen);
            let exited = ctx.run(Default::default(), |ctx| {
                display.set_image_fullscreen(ctx, false);
                display.update(ctx);
            });
            let commands = |output: egui::FullOutput| {
                output.viewport_output[&egui::ViewportId::ROOT].commands.clone()
            };
            assert_eq!(commands(entered).contains(&Cmd::Fullscreen(true)), !fullscreen);
            assert_eq!(commands(exited).contains(&Cmd::Fullscreen(false)), !fullscreen);
            assert_eq!(display.applied, Some(fullscreen));
        }
    }

    #[test]
    fn image_view_stays_visible_until_the_window_is_restored() {
        let ctx = Context::default();
        let mut display = Display {
            image_fullscreen: true,
            applied: Some(true),
            placement: Some([80.0, 80.0, 1000.0, 700.0]),
            ..Default::default()
        };
        let frame = |display: &mut Display, fullscreen, size| {
            let mut input = egui::RawInput::default();
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.fullscreen = Some(fullscreen);
            viewport.inner_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
            ctx.run(input, |ctx| display.update(ctx))
        };
        display.set_image_fullscreen(&ctx, false);
        let screen = egui::vec2(1920.0, 1080.0);
        let exit = frame(&mut display, true, screen);
        assert!(exit.viewport_output[&egui::ViewportId::ROOT].commands.contains(&Cmd::Fullscreen(false)));
        assert!(display.shows_image_view());
        for _ in 0..3 {
            let _ = frame(&mut display, false, screen);
            assert!(display.shows_image_view());
        }
        let _ = frame(&mut display, false, egui::vec2(1000.0, 700.0));
        assert!(!display.shows_image_view());
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
