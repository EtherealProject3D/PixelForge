//! Transient, pixel-accurate crop selection and an independent preview camera.
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureId, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZoomMode {
    Workspace,
    ImageOnly,
}

pub struct Crop {
    pub source: usize,
    pub rect: Rect,
    pub locked: bool,
    pub preset: usize,
    pub zoom_mode: ZoomMode,
    aspect: f32,
    zoom: f32,
    zoom_request: f32,
    pan: Vec2,
    drag: Option<(i8, i8, Pos2, Rect)>,
}
impl Crop {
    pub fn new(source: usize, size: [u32; 2]) -> Self {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(size[0] as f32, size[1] as f32));
        Self {
            source,
            rect: if bounds.width() < 2.0 || bounds.height() < 2.0 {
                bounds
            } else {
                bounds.shrink2(bounds.size() * 0.1)
            },
            locked: false,
            preset: 0,
            zoom_mode: ZoomMode::Workspace,
            aspect: bounds.aspect_ratio(),
            zoom: 1.0,
            zoom_request: 1.0,
            pan: Vec2::ZERO,
            drag: None,
        }
    }
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.zoom_request = 1.0;
        self.pan = Vec2::ZERO;
        self.drag = None;
    }
    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom_request *= factor;
    }
    pub fn set_aspect(&mut self, aspect: f32, size: [u32; 2]) {
        self.aspect = aspect;
        self.locked = true;
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(size[0] as f32, size[1] as f32));
        let w = self.rect.width().min(self.rect.height() * aspect).max(1.0);
        let w = w.min(bounds.width()).min(bounds.height() * aspect);
        self.rect = bounded(
            Rect::from_center_size(self.rect.center(), Vec2::new(w, w / aspect)),
            bounds,
        );
    }
    pub fn lock_current(&mut self) {
        self.aspect = self.rect.aspect_ratio();
    }
    pub fn locked_ratio(&self) -> f32 {
        self.aspect
    }

    /// Set actual source-pixel dimensions, retaining the center wherever the image allows.
    /// Only the edited axis drives a locked ratio; the second axis rounds to one pixel.
    pub fn set_pixel_size(&mut self, width: u32, height: u32, width_changed: bool, size: [u32; 2]) {
        let mut w = width.clamp(1, size[0]);
        let mut h = height.clamp(1, size[1]);
        if self.locked {
            if width_changed {
                h = (w as f32 / self.aspect).round().max(1.0) as u32;
                if h > size[1] {
                    h = size[1];
                    w = (h as f32 * self.aspect).round().max(1.0) as u32;
                }
            } else {
                w = (h as f32 * self.aspect).round().max(1.0) as u32;
                if w > size[0] {
                    w = size[0];
                    h = (w as f32 / self.aspect).round().max(1.0) as u32;
                }
            }
        }
        w = w.clamp(1, size[0]);
        h = h.clamp(1, size[1]);
        let center = self.rect.center();
        let x = (center.x - w as f32 / 2.0)
            .round()
            .clamp(0.0, (size[0] - w) as f32);
        let y = (center.y - h as f32 / 2.0)
            .round()
            .clamp(0.0, (size[1] - h) as f32);
        self.rect = Rect::from_min_size(egui::pos2(x, y), egui::vec2(w as f32, h as f32));
        self.drag = None;
    }
    pub fn pixels(&self, size: [u32; 2]) -> [u32; 4] {
        let x = (self.rect.min.x.round() as u32).min(size[0].saturating_sub(1));
        let y = (self.rect.min.y.round() as u32).min(size[1].saturating_sub(1));
        let right = (self.rect.max.x.round() as u32).clamp(x + 1, size[0]);
        let bottom = (self.rect.max.y.round() as u32).clamp(y + 1, size[1]);
        [x, y, right - x, bottom - y]
    }
    pub fn show(&mut self, ui: &mut Ui, texture: TextureId, size: [u32; 2]) {
        let (canvas, response) = ui.allocate_exact_size(
            ui.available_size().max(Vec2::splat(1.0)),
            Sense::click_and_drag(),
        );
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(size[0] as f32, size[1] as f32));
        let fit = (canvas.width() / bounds.width()).min(canvas.height() / bounds.height()) * 0.92;
        let mut scale = (fit * self.zoom).max(0.00001);
        let mut image = Rect::from_center_size(canvas.center() + self.pan, bounds.size() * scale);
        let screen_crop = Rect::from_min_max(
            image.min + self.rect.min.to_vec2() * scale,
            image.min + self.rect.max.to_vec2() * scale,
        );
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let over = pointer.is_some_and(|p| canvas.contains(p));
        if (over || self.zoom_request != 1.0) && self.drag.is_none() {
            let scroll = if over {
                ui.input(|i| i.smooth_scroll_delta.y)
            } else {
                0.0
            };
            let middle = over && ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
            if middle || scroll != 0.0 || self.zoom_request != 1.0 {
                if scroll != 0.0 || self.zoom_request != 1.0 {
                    let old_scale = scale;
                    let minimum = if self.zoom_mode == ZoomMode::ImageOnly {
                        (screen_crop.width() / bounds.width())
                            .max(screen_crop.height() / bounds.height())
                            / fit
                    } else {
                        0.05
                    };
                    self.zoom = (self.zoom * (scroll * 0.002).exp() * self.zoom_request)
                        .clamp(minimum.max(0.05), 32.0_f32.max(minimum));
                    scale = fit * self.zoom;
                    let anchor = if self.zoom_request != 1.0 {
                        screen_crop.center()
                    } else {
                        pointer.unwrap_or(canvas.center())
                    };
                    self.zoom_request = 1.0;
                    let min = anchor + (image.min - anchor) * (scale / old_scale);
                    image = Rect::from_min_size(min, bounds.size() * scale);
                }
                if middle {
                    image = image.translate(ui.input(|i| i.pointer.delta()));
                }
                if self.zoom_mode == ZoomMode::ImageOnly {
                    // Image-only mode changes the selected pixels inside a fixed screen frame.
                    image = image.translate(cover_translation(image, screen_crop));
                    self.rect = Rect::from_min_max(
                        ((screen_crop.min - image.min) / scale).to_pos2(),
                        ((screen_crop.max - image.min) / scale).to_pos2(),
                    );
                    self.rect = bounded(self.rect, bounds);
                }
                // Workspace mode leaves the source-pixel rectangle untouched.
                self.pan = image.center() - canvas.center();
                ui.ctx().set_cursor_icon(if middle {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Default
                });
            }
        }
        let crop = Rect::from_min_max(
            image.min + self.rect.min.to_vec2() * scale,
            image.min + self.rect.max.to_vec2() * scale,
        );
        let handle_points = handles(crop);
        let hovered = pointer
            .and_then(|p| {
                handle_points
                    .iter()
                    .find(|(_, _, point)| point.distance(p) < 10.0)
                    .map(|(x, y, _)| (*x, *y))
            })
            .or_else(|| pointer.filter(|p| crop.contains(*p)).map(|_| (0, 0)));
        if over && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
            if let (Some((x, y)), Some(p)) = (hovered, pointer) {
                self.drag = Some((x, y, p, self.rect));
            }
        }
        if !ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
            self.drag = None;
        }
        if let Some((x, y, start, rect)) = self.drag {
            if let Some(p) = pointer {
                let delta = (p - start) / scale;
                self.rect = resize(
                    rect,
                    delta,
                    (x, y),
                    bounds,
                    self.locked.then_some(self.aspect),
                );
            }
        }
        if let Some((x, y)) = self
            .drag
            .map(|(x, y, _, _)| (x, y))
            .or(hovered)
            .filter(|_| over && !ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle)))
        {
            ui.ctx().set_cursor_icon(match (x, y) {
                (0, 0) => egui::CursorIcon::Move,
                (0, _) => egui::CursorIcon::ResizeVertical,
                (_, 0) => egui::CursorIcon::ResizeHorizontal,
                (-1, -1) | (1, 1) => egui::CursorIcon::ResizeNwSe,
                _ => egui::CursorIcon::ResizeNeSw,
            });
        }
        if response.double_clicked_by(egui::PointerButton::Middle) {
            self.reset_view();
        }
        let crop = Rect::from_min_max(
            image.min + self.rect.min.to_vec2() * scale,
            image.min + self.rect.max.to_vec2() * scale,
        );
        let painter = ui.painter().with_clip_rect(canvas);
        painter.rect_filled(canvas, 0.0, Color32::from_gray(18));
        painter.image(
            texture,
            image,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let shade = Color32::from_black_alpha(155);
        for rect in [
            Rect::from_min_max(
                canvas.min,
                egui::pos2(
                    canvas.right(),
                    crop.top().clamp(canvas.top(), canvas.bottom()),
                ),
            ),
            Rect::from_min_max(
                egui::pos2(
                    canvas.left(),
                    crop.bottom().clamp(canvas.top(), canvas.bottom()),
                ),
                canvas.max,
            ),
            Rect::from_min_max(
                egui::pos2(canvas.left(), crop.top()),
                egui::pos2(crop.left(), crop.bottom()),
            ),
            Rect::from_min_max(
                egui::pos2(crop.right(), crop.top()),
                egui::pos2(canvas.right(), crop.bottom()),
            ),
        ] {
            if rect.is_positive() {
                painter.rect_filled(rect, 0.0, shade);
            }
        }
        painter.rect_stroke(
            crop,
            0.0,
            Stroke::new(2.0_f32, Color32::WHITE),
            StrokeKind::Inside,
        );
        for fraction in [1.0 / 3.0, 2.0 / 3.0] {
            let x = crop.left() + crop.width() * fraction;
            let y = crop.top() + crop.height() * fraction;
            let stroke = Stroke::new(1.0_f32, Color32::from_white_alpha(120));
            painter.line_segment(
                [egui::pos2(x, crop.top()), egui::pos2(x, crop.bottom())],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(crop.left(), y), egui::pos2(crop.right(), y)],
                stroke,
            );
        }
        for (_, _, point) in handles(crop) {
            painter.rect_filled(
                Rect::from_center_size(point, Vec2::splat(8.0)),
                1.0,
                Color32::WHITE,
            );
        }
    }
}
fn handles(rect: Rect) -> [(i8, i8, Pos2); 8] {
    [
        (-1, -1, rect.left_top()),
        (0, -1, rect.center_top()),
        (1, -1, rect.right_top()),
        (-1, 0, rect.left_center()),
        (1, 0, rect.right_center()),
        (-1, 1, rect.left_bottom()),
        (0, 1, rect.center_bottom()),
        (1, 1, rect.right_bottom()),
    ]
}
fn cover_translation(image: Rect, crop: Rect) -> Vec2 {
    let x = if image.left() > crop.left() {
        crop.left() - image.left()
    } else if image.right() < crop.right() {
        crop.right() - image.right()
    } else {
        0.0
    };
    let y = if image.top() > crop.top() {
        crop.top() - image.top()
    } else if image.bottom() < crop.bottom() {
        crop.bottom() - image.bottom()
    } else {
        0.0
    };
    Vec2::new(x, y)
}
fn bounded(rect: Rect, bounds: Rect) -> Rect {
    let rect = Rect::from_min_size(rect.min, rect.size().min(bounds.size()));
    let x = rect.left().clamp(
        bounds.left(),
        (bounds.right() - rect.width()).max(bounds.left()),
    );
    let y = rect.top().clamp(
        bounds.top(),
        (bounds.bottom() - rect.height()).max(bounds.top()),
    );
    Rect::from_min_size(egui::pos2(x, y), rect.size())
}
pub(crate) fn resize(
    start: Rect,
    delta: Vec2,
    handle: (i8, i8),
    bounds: Rect,
    aspect: Option<f32>,
) -> Rect {
    let (x, y) = handle;
    if handle == (0, 0) {
        return bounded(start.translate(delta), bounds);
    }
    let mut rect = start;
    if x < 0 {
        rect.min.x = (start.min.x + delta.x).clamp(bounds.left(), start.max.x - 1.0);
    }
    if x > 0 {
        rect.max.x = (start.max.x + delta.x).clamp(start.min.x + 1.0, bounds.right());
    }
    if y < 0 {
        rect.min.y = (start.min.y + delta.y).clamp(bounds.top(), start.max.y - 1.0);
    }
    if y > 0 {
        rect.max.y = (start.max.y + delta.y).clamp(start.min.y + 1.0, bounds.bottom());
    }
    if let Some(ratio) = aspect {
        let anchor = egui::pos2(
            if x < 0 {
                start.right()
            } else if x > 0 {
                start.left()
            } else {
                start.center().x
            },
            if y < 0 {
                start.bottom()
            } else if y > 0 {
                start.top()
            } else {
                start.center().y
            },
        );
        let wanted = if x == 0 {
            rect.height() * ratio
        } else if y == 0 {
            rect.width()
        } else if delta.x.abs() >= delta.y.abs() * ratio {
            rect.width()
        } else {
            rect.height() * ratio
        };
        let max_w = if x < 0 {
            anchor.x - bounds.left()
        } else if x > 0 {
            bounds.right() - anchor.x
        } else {
            2.0 * (anchor.x - bounds.left()).min(bounds.right() - anchor.x)
        };
        let max_h = if y < 0 {
            anchor.y - bounds.top()
        } else if y > 0 {
            bounds.bottom() - anchor.y
        } else {
            2.0 * (anchor.y - bounds.top()).min(bounds.bottom() - anchor.y)
        };
        let w = wanted.max(ratio.max(1.0)).min(max_w).min(max_h * ratio);
        let h = w / ratio;
        let min = egui::pos2(
            if x < 0 {
                anchor.x - w
            } else if x > 0 {
                anchor.x
            } else {
                anchor.x - w / 2.0
            },
            if y < 0 {
                anchor.y - h
            } else if y > 0 {
                anchor.y
            } else {
                anchor.y - h / 2.0
            },
        );
        rect = Rect::from_min_size(min, egui::vec2(w, h));
    }
    bounded(rect, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_dimensions_are_exact_and_stay_within_source_edges() {
        let mut crop = Crop::new(0, [1001, 777]);
        crop.rect = Rect::from_min_max(egui::pos2(800.5, 600.25), egui::pos2(1000.5, 776.25));
        crop.set_pixel_size(101, 203, true, [1001, 777]);
        let [x, y, w, h] = crop.pixels([1001, 777]);
        assert_eq!((w, h), (101, 203));
        assert!(x + w <= 1001 && y + h <= 777);
        crop.set_pixel_size(0, u32::MAX, false, [1001, 777]);
        assert_eq!(&crop.pixels([1001, 777])[2..], &[1, 777]);
    }
    #[test]
    fn numeric_locked_sizes_follow_either_axis_and_keep_preset_ratio() {
        let mut crop = Crop::new(0, [1000, 600]);
        crop.set_aspect(16.0 / 9.0, [1000, 600]);
        crop.set_pixel_size(640, 10, true, [1000, 600]);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[640, 360]);
        crop.set_pixel_size(10, 270, false, [1000, 600]);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[480, 270]);
        crop.set_pixel_size(10, 600, false, [1000, 600]);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[1000, 563]);
        assert_eq!(crop.locked_ratio(), 16.0 / 9.0);
    }
    #[test]
    fn zoom_modes_change_the_view_or_the_selected_pixels_without_jumping_on_switch() {
        let ctx = egui::Context::default();
        let mut crop = Crop::new(0, [1000, 600]);
        crop.set_pixel_size(600, 300, true, [1000, 600]);
        let frame = |crop: &mut Crop| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 800.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| crop.show(ui, TextureId::Managed(0), [1000, 600]));
            });
        };
        frame(&mut crop);
        let original = crop.rect;
        crop.zoom_by(2.0);
        frame(&mut crop);
        assert_eq!(crop.zoom, 2.0);
        assert_eq!(crop.rect, original);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[600, 300]);
        crop.zoom_mode = ZoomMode::ImageOnly;
        frame(&mut crop);
        assert_eq!(crop.rect, original); // Changing the option alone doesn't change the crop.
        crop.zoom_by(2.0);
        frame(&mut crop);
        assert_eq!(crop.zoom, 4.0);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[300, 150]);
        let selected = crop.rect;
        crop.zoom_mode = ZoomMode::Workspace;
        crop.zoom_by(0.5);
        frame(&mut crop);
        assert_eq!(crop.rect, selected);
        assert_eq!(&crop.pixels([1000, 600])[2..], &[300, 150]);
    }
    #[test]
    fn free_resize_and_move_stay_inside_image() {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        let start = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(500.0, 400.0));
        let free = resize(start, egui::vec2(200.0, 30.0), (1, 1), bounds, None);
        assert_eq!(free.size(), egui::vec2(600.0, 330.0));
        let moved = resize(start, egui::vec2(2000.0, -500.0), (0, 0), bounds, None);
        assert_eq!(moved.size(), start.size());
        assert!(bounds.contains_rect(moved));
        let tiny = resize(start, Vec2::splat(-500.0), (1, 1), bounds, None);
        assert_eq!(tiny.size(), Vec2::splat(1.0));
    }
    #[test]
    fn every_locked_handle_keeps_ratio_at_image_edges() {
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let start = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(500.0, 325.0));
        for (x, y, _) in handles(start) {
            for delta in [
                egui::vec2(150.0, 90.0),
                egui::vec2(-900.0, -900.0),
                Vec2::splat(2000.0),
            ] {
                let rect = resize(start, delta, (x, y), bounds, Some(16.0 / 9.0));
                assert!((rect.aspect_ratio() - 16.0 / 9.0).abs() < 0.001);
                assert!(bounds.expand(0.001).contains_rect(rect));
            }
        }
    }
    #[test]
    fn mouse_handles_pan_and_zoom_adjust_the_preview_without_losing_the_frame() {
        let ctx = egui::Context::default();
        let mut crop = Crop::new(0, [1000, 600]);
        crop.zoom_mode = ZoomMode::ImageOnly;
        let mut canvas = Rect::NOTHING;
        let frame = |crop: &mut Crop, canvas: &mut Rect, events| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 800.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    *canvas = Rect::from_min_size(ui.cursor().min, ui.available_size());
                    crop.show(ui, TextureId::Managed(0), [1000, 600]);
                });
            });
        };
        let screen_frame = |crop: &Crop, canvas: Rect| {
            let fit = (canvas.width() / 1000.0).min(canvas.height() / 600.0) * 0.92;
            let scale = fit * crop.zoom;
            let min = canvas.center() + crop.pan - egui::vec2(1000.0, 600.0) * scale / 2.0;
            Rect::from_min_max(
                min + crop.rect.min.to_vec2() * scale,
                min + crop.rect.max.to_vec2() * scale,
            )
        };
        let button = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut crop, &mut canvas, vec![]);
        let start = screen_frame(&crop, canvas).right_bottom();
        frame(
            &mut crop,
            &mut canvas,
            vec![
                egui::Event::PointerMoved(start),
                button(start, egui::PointerButton::Primary, true),
            ],
        );
        let end = start + egui::vec2(40.0, 20.0);
        frame(&mut crop, &mut canvas, vec![egui::Event::PointerMoved(end)]);
        assert!(crop.rect.width() > 800.0);
        assert!(crop.rect.height() > 480.0);
        frame(
            &mut crop,
            &mut canvas,
            vec![button(end, egui::PointerButton::Primary, false)],
        );
        let before = screen_frame(&crop, canvas);
        let center = before.center();
        frame(
            &mut crop,
            &mut canvas,
            vec![
                egui::Event::PointerMoved(center),
                button(center, egui::PointerButton::Middle, true),
            ],
        );
        let old_pixels = crop.rect;
        frame(
            &mut crop,
            &mut canvas,
            vec![egui::Event::PointerMoved(center + egui::vec2(20.0, 10.0))],
        );
        assert!(crop.rect.min.x < old_pixels.min.x);
        assert!((screen_frame(&crop, canvas).min - before.min).length() < 0.01);
        frame(
            &mut crop,
            &mut canvas,
            vec![button(center, egui::PointerButton::Middle, false)],
        );
        let old_width = crop.rect.width();
        frame(
            &mut crop,
            &mut canvas,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100.0),
                modifiers: Default::default(),
            }],
        );
        assert!(crop.rect.width() < old_width);
        assert!((screen_frame(&crop, canvas).min - before.min).length() < 0.01);
    }
    #[test]
    fn pixel_crop_is_not_rounded_to_generator_grid() {
        let mut crop = Crop::new(0, [1001, 777]);
        crop.rect = Rect::from_min_max(egui::pos2(3.0, 7.0), egui::pos2(104.0, 210.0));
        assert_eq!(crop.pixels([1001, 777]), [3, 7, 101, 203]);
    }
}
