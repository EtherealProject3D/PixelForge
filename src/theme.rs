//! Shared visual styling; tool layouts and image processing remain independent.
use egui::{Color32, CornerRadius, Stroke};

pub const CANVAS: Color32 = Color32::from_rgb(18, 22, 29);
const PANEL: Color32 = Color32::from_rgb(30, 36, 46);
const TEXT: Color32 = Color32::from_rgb(224, 231, 241);
const ACCENT: Color32 = Color32::from_rgb(49, 113, 185);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = Color32::from_rgb(34, 41, 53);
    visuals.extreme_bg_color = CANVAS;
    visuals.text_edit_bg_color = Some(Color32::from_rgb(22, 28, 37));
    visuals.faint_bg_color = Color32::from_rgb(36, 44, 56);
    visuals.weak_text_color = Some(Color32::from_rgb(166, 180, 199));
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.text_cursor.stroke = Stroke::new(2.0_f32, Color32::WHITE);
    visuals.text_cursor.blink = false;
    visuals.hyperlink_color = Color32::from_rgb(116, 183, 255);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(7);
    visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(65, 78, 96));
    visuals.collapsing_header_frame = true;
    visuals.slider_trailing_fill = true;
    for (widget, fill, border) in [
        (
            &mut visuals.widgets.noninteractive,
            PANEL,
            Color32::from_rgb(53, 64, 80),
        ),
        (
            &mut visuals.widgets.inactive,
            Color32::from_rgb(43, 53, 68),
            Color32::from_rgb(68, 83, 103),
        ),
        (
            &mut visuals.widgets.hovered,
            Color32::from_rgb(54, 73, 96),
            Color32::from_rgb(114, 170, 228),
        ),
        (
            &mut visuals.widgets.active,
            Color32::from_rgb(42, 88, 137),
            Color32::from_rgb(131, 194, 255),
        ),
        (
            &mut visuals.widgets.open,
            Color32::from_rgb(43, 61, 82),
            Color32::from_rgb(98, 147, 199),
        ),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0_f32, border);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widget.corner_radius = CornerRadius::same(5);
        widget.expansion = 0.0;
    }
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.button_padding = egui::vec2(9.0, 5.0);
        style.animation_time = 0.12;
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_width = 10.0;
    });
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, 19.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 2, Color32::from_rgb(116, 183, 255));
        ui.heading(egui::RichText::new(title).color(TEXT).strong());
    });
}

/// Edit-panel section heading: accent bar, title, then the disclosure arrow.
pub fn collapsible_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    collapsible_section_reveal(ui, title, true, false, body)
}

pub fn collapsible_section_reveal<R>(
    ui: &mut egui::Ui,
    title: &str,
    default_open: bool,
    reveal: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let id = ui.make_persistent_id(("edit_section", title));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    if reveal { state.set_open(true); }
    let text = ui.painter().layout_no_wrap(
        title.into(),
        egui::TextStyle::Heading.resolve(ui.style()),
        TEXT,
    );
    let height = (text.size().y + 6.0).max(ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if response.clicked() {
        state.toggle(ui);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::CollapsingHeader,
            ui.is_enabled(),
            state.is_open(),
            title,
        )
    });
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let bar = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 1.5, rect.center().y),
        egui::vec2(3.0, 19.0),
    );
    ui.painter()
        .rect_filled(bar, 2.0, Color32::from_rgb(116, 183, 255));
    let pos = egui::pos2(
        rect.left() + 3.0 + ui.spacing().item_spacing.x,
        rect.center().y - text.size().y / 2.0,
    );
    let center = egui::pos2(
        pos.x + text.size().x + ui.spacing().item_spacing.x + 5.0,
        rect.center().y,
    );
    ui.painter().galley(pos, text, TEXT);
    let points = if state.is_open() {
        vec![
            center + egui::vec2(-4.0, -2.5),
            center + egui::vec2(4.0, -2.5),
            center + egui::vec2(0.0, 3.5),
        ]
    } else {
        vec![
            center + egui::vec2(-2.5, -4.0),
            center + egui::vec2(3.5, 0.0),
            center + egui::vec2(-2.5, 4.0),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(points, TEXT, Stroke::NONE));
    state
        .show_body_unindented(ui, body)
        .map(|response| response.inner)
}

pub fn primary(label: &str) -> egui::Button<'_> {
    primary_text(egui::RichText::new(label))
}

pub fn primary_text<'a>(label: egui::RichText) -> egui::Button<'a> {
    egui::Button::new(label.color(Color32::WHITE).strong())
        .fill(ACCENT)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(87, 149, 218)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_light_mode_cannot_replace_pixelforge_colors() {
        let ctx = egui::Context::default();
        super::apply(&ctx);
        for system_theme in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(system_theme),
                    ..Default::default()
                },
                |ctx| {
                    assert!(ctx.style().visuals.dark_mode);
                    assert_eq!(ctx.style().visuals.panel_fill, super::PANEL);
                    assert_eq!(ctx.style().visuals.selection.bg_fill, super::ACCENT);
                },
            );
        }
    }
}
