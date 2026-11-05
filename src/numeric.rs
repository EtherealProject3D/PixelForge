//! Numeric widgets keep native click/drag entry and add consistent wheel precision.
use egui::{Response, Ui, Widget, emath::Numeric};
use std::ops::RangeInclusive;
type Formatter<'a> = Box<dyn Fn(f64, RangeInclusive<usize>) -> String + 'a>;
pub struct Slider<'a> {
    value: Box<dyn FnMut(Option<f64>) -> f64 + 'a>,
    range: RangeInclusive<f64>,
    integral: bool,
    show: bool,
    suffix: String,
    text: String,
    format: Option<Formatter<'a>>,
    percent: bool,
}
impl<'a> Slider<'a> {
    pub fn new<N: Numeric>(value: &'a mut N, range: RangeInclusive<N>) -> Self {
        let range = range.start().to_f64()..=range.end().to_f64();
        Self {
            value: Box::new(move |v| {
                if let Some(v) = v {
                    *value = N::from_f64(v);
                }
                value.to_f64()
            }),
            percent: !N::INTEGRAL && *range.start() >= 0.0 && *range.end() <= 1.0,
            range,
            integral: N::INTEGRAL,
            show: true,
            suffix: String::new(),
            text: String::new(),
            format: None,
        }
    }
    // Fine wheel/drag steps supersede legacy coarse steps without changing value limits.
    pub fn step_by(self, _: f64) -> Self {
        self
    }
    pub fn percent(mut self) -> Self {
        self.percent = true;
        self
    }
    pub fn show_value(mut self, value: bool) -> Self {
        self.show = value;
        self
    }
    pub fn suffix(mut self, value: impl ToString) -> Self {
        self.suffix = value.to_string();
        self
    }
    pub fn text(mut self, value: impl ToString) -> Self {
        self.text = value.to_string();
        self
    }
    pub fn custom_formatter(
        mut self,
        value: impl Fn(f64, RangeInclusive<usize>) -> String + 'a,
    ) -> Self {
        self.format = Some(Box::new(value));
        self
    }
}
fn fine_input(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> Response) -> Response {
    // egui's DragValue uses Shift for fine dragging. Remap it only inside numeric widgets.
    let original = ui.input(|i| i.modifiers);
    ui.ctx().input_mut(|i| {
        i.modifiers.shift = original.ctrl;
        i.modifiers.ctrl = false;
        i.modifiers.command = false;
    });
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(ui.available_rect_before_wrap())
            .layout(*ui.layout()),
    );
    child.style_mut().explanation_tooltips = false;
    let response = body(&mut child);
    ui.advance_cursor_after_rect(child.min_rect());
    ui.ctx().input_mut(|i| i.modifiers = original);
    response
}
fn wheel(
    ui: &Ui,
    response: &mut Response,
    current: f64,
    range: RangeInclusive<f64>,
    integral: bool,
    scale: f64,
    quantum: f64,
) -> Option<f64> {
    if !response.hovered() || !response.enabled() {
        return None;
    }
    let (delta, mods) = ui.input(|i| {
        let delta = i
            .events
            .iter()
            .filter_map(|e| {
                if let egui::Event::MouseWheel { unit, delta, .. } = e {
                    Some(match unit {
                        egui::MouseWheelUnit::Line => delta.y as f64,
                        egui::MouseWheelUnit::Point => delta.y as f64 / 40.0,
                        egui::MouseWheelUnit::Page => delta.y.signum() as f64,
                    })
                } else {
                    None
                }
            })
            .sum::<f64>();
        (delta, i.modifiers)
    });
    // egui smooths wheel input across later frames without new MouseWheel events.
    // Consume those frames too while over a numeric control, before the parent scroll area.
    ui.ctx().input_mut(|i| {
        i.raw_scroll_delta = egui::Vec2::ZERO;
        i.smooth_scroll_delta = egui::Vec2::ZERO;
        i.events.retain(|e| !matches!(e, egui::Event::MouseWheel { .. }));
    });
    if delta == 0.0 { return None; }
    let id = response.id.with("wheel_remainder");
    let accumulated = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(0.0) + delta;
    let notches = accumulated.trunc();
    ui.ctx()
        .data_mut(|d| d.insert_temp(id, accumulated - notches));
    if notches == 0.0 {
        return None;
    }
    let step = (if mods.shift {
        5.0
    } else if mods.ctrl && !integral {
        0.1
    } else {
        1.0
    }) * quantum;
    let value = ((current * scale + notches * step) * 10.0).round() / 10.0 / scale;
    let value = value.clamp(*range.start(), *range.end());
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            response.id.with("wheel_delta"),
            (notches * step / scale) as i64,
        )
    });
    if value != current || (integral && current.abs() > 9_007_199_254_740_992.0) {
        response.mark_changed();
        Some(value)
    } else {
        None
    }
}
impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let scale = if self.percent { 100.0 } else { 1.0 };
        let original = (self.value)(None);
        let mut value = original * scale;
        let range = *self.range.start() * scale..=*self.range.end() * scale;
        let integral = self.integral;
        let mut response = fine_input(ui, |ui| {
            let mut slider = egui::Slider::new(&mut value, range)
                .show_value(self.show)
                .text(&self.text)
                .step_by(if integral { 1.0 } else { 0.1 })
                .max_decimals(if integral { 0 } else { 1 })
                .min_decimals(if integral { 0 } else { 1 })
                .drag_value_speed(1.0)
                .suffix(if self.percent { "%" } else { &self.suffix });
            if let Some(format) = self.format.as_ref().filter(|_| !self.percent) {
                slider = slider.custom_formatter(|v, r| format(v, r));
            }
            ui.add(slider)
        });
        let precision = response.id.with("precision_drag");
        if ui.input(|i| i.modifiers.ctrl && i.pointer.button_down(egui::PointerButton::Primary))
            && (response.hovered() || response.dragged())
            && ui.input(|i| i.pointer.press_origin()).is_some_and(|p| {
                p.x >= response.rect.left()
                    && p.x < response.rect.left() + ui.spacing().slider_width
            })
        {
            let old = ui
                .data(|d| d.get_temp::<f64>(precision))
                .unwrap_or(original * scale);
            let delta = if response.dragged() {
                ui.input(|i| i.pointer.delta().x) as f64
            } else {
                0.0
            };
            let precise = (old
                + delta / (ui.spacing().slider_width.max(1.0) as f64)
                    * (*self.range.end() - *self.range.start())
                    * scale
                    / 10.0)
                .clamp(*self.range.start() * scale, *self.range.end() * scale);
            ui.ctx().data_mut(|d| d.insert_temp(precision, precise));
            value = if integral {
                precise.round()
            } else {
                (precise * 10.0).round() / 10.0
            };
        } else {
            ui.ctx().data_mut(|d| d.remove::<f64>(precision));
        }
        if let Some(next) = wheel(
            ui,
            &mut response,
            value / scale,
            self.range.clone(),
            integral,
            scale,
            1.0,
        ) {
            value = next * scale;
        }
        if response.changed() {
            (self.value)(Some(value / scale));
        }
        crate::interaction::numeric_text(ui, &response);
        response.on_hover_text("Drag to adjust; Ctrl+drag for precision. Wheel: 1; Ctrl+wheel: 0.1; Shift+wheel: 5 (displayed units).")
    }
}
pub struct DragValue<'a> {
    value: Box<dyn FnMut(Option<f64>) -> f64 + 'a>,
    exact_step: Box<dyn FnMut(i64, &RangeInclusive<f64>) -> f64 + 'a>,
    integral: bool,
    range: RangeInclusive<f64>,
    speed: f64,
    suffix: String,
    editing: bool,
}
impl<'a> DragValue<'a> {
    pub fn new<N: Numeric>(value: &'a mut N) -> Self {
        let shared = std::rc::Rc::new(std::cell::RefCell::new(value));
        let exact = shared.clone();
        Self {
            value: Box::new(move |v| {
                let mut value = shared.borrow_mut();
                if let Some(v) = v {
                    **value = N::from_f64(v);
                }
                (**value).to_f64()
            }),
            exact_step: Box::new(move |delta, range| {
                let mut value = exact.borrow_mut();
                if let Some(value) = (&mut **value as &mut dyn std::any::Any).downcast_mut::<u64>()
                {
                    *value = if delta >= 0 {
                        value.saturating_add(delta as u64)
                    } else {
                        value.saturating_sub(delta.unsigned_abs())
                    };
                    *value = (*value).clamp(*range.start() as u64, *range.end() as u64);
                    *value as f64
                } else {
                    **value = N::from_f64(
                        ((**value).to_f64() + delta as f64).clamp(*range.start(), *range.end()),
                    );
                    (**value).to_f64()
                }
            }),
            integral: N::INTEGRAL,
            range: N::MIN.to_f64()..=N::MAX.to_f64(),
            speed: 1.0,
            suffix: String::new(),
            editing: true,
        }
    }
    pub fn range<N: Numeric>(mut self, range: RangeInclusive<N>) -> Self {
        self.range = range.start().to_f64()..=range.end().to_f64();
        self
    }
    pub fn speed(mut self, speed: impl Into<f64>) -> Self {
        self.speed = speed.into();
        self
    }
    pub fn suffix(mut self, suffix: impl ToString) -> Self {
        self.suffix = suffix.to_string();
        self
    }
    pub fn update_while_editing(mut self, value: bool) -> Self {
        self.editing = value;
        self
    }
}
impl Widget for DragValue<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let mut value = (self.value)(None);
        let mut response = fine_input(ui, |ui| {
            ui.add(
                egui::DragValue::new(&mut value)
                    .range(self.range.clone())
                    .speed(self.speed)
                    .max_decimals(if self.integral { 0 } else { 1 })
                    .min_decimals(if self.integral { 0 } else { 1 })
                    .suffix(&self.suffix)
                    .update_while_editing(self.editing),
            )
        });
        if response.changed() {
            (self.value)(Some(value));
        }
        if let Some(next) = wheel(
            ui,
            &mut response,
            value,
            self.range.clone(),
            self.integral,
            1.0,
            if self.integral {
                self.speed.max(1.0)
            } else {
                1.0
            },
        ) {
            if self.integral {
                let delta = ui
                    .data(|d| d.get_temp::<i64>(response.id.with("wheel_delta")))
                    .unwrap_or(0);
                (self.exact_step)(delta, &self.range);
            } else {
                (self.value)(Some(next));
            }
        }
        crate::interaction::numeric_text(ui, &response);
        response.on_hover_text("Click to type or drag to adjust. Ctrl+drag: precision. Wheel: 1; Ctrl+wheel: 0.1; Shift+wheel: 5.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_wheel_does_not_scroll_parent_on_smoothed_followup_frames() {
        let ctx = egui::Context::default();
        let mut value = 50_u32;
        let mut rect = egui::Rect::NOTHING;
        for frame in 0..20 {
            let mut events = vec![egui::Event::PointerMoved(rect.center())];
            if frame == 1 {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, -3.0),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            if frame == 0 { events.clear(); }
            let _ = ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let area = egui::ScrollArea::vertical().show(ui, |ui| {
                        rect = ui.add(Slider::new(&mut value, 0..=100)).rect;
                        ui.add_space(1000.0);
                    });
                    assert_eq!(area.state.offset.y, 0.0, "parent scrolled on frame {frame}");
                });
            });
        }
        assert_eq!(value, 47);
    }
    #[test]
    fn wheel_uses_displayed_units_modifiers_and_stops_parent_scrolling() {
        let ctx = egui::Context::default();
        let mut value = 0.5_f32;
        let mut rect = egui::Rect::NOTHING;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rect = ui.add(Slider::new(&mut value, 0.0..=1.0)).rect;
            });
        });
        for (modifiers, expected) in [
            (egui::Modifiers::NONE, 0.51),
            (
                egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
                0.511,
            ),
            (
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                0.561,
            ),
        ] {
            let events = vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, 1.0),
                    modifiers,
                },
            ];
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add(Slider::new(&mut value, 0.0..=1.0));
                        assert_eq!(ui.input(|i| i.raw_scroll_delta), egui::Vec2::ZERO);
                    });
                },
            );
            assert!(
                (value - expected).abs() < 0.00001,
                "expected {expected}, got {value}"
            );
        }
    }
    #[test]
    fn integer_fields_ignore_decimal_mode_and_outside_scroll_is_not_consumed() {
        let ctx = egui::Context::default();
        let mut value = 10_u32;
        let mut rect = egui::Rect::NOTHING;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rect = ui.add(DragValue::new(&mut value).range(0..=100)).rect;
            });
        });
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            modifiers,
        };
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![egui::Event::PointerMoved(rect.center()), wheel.clone()],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(DragValue::new(&mut value).range(0..=100));
                });
            },
        );
        assert_eq!(value, 11);
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(egui::pos2(500.0, 400.0)), wheel],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(DragValue::new(&mut value).range(0..=100));
                    assert_ne!(ui.input(|i| i.raw_scroll_delta), egui::Vec2::ZERO);
                });
            },
        );
        assert_eq!(value, 11);
    }
    #[test]
    fn large_seed_changes_by_exactly_one_and_grid_fields_move_by_valid_increments() {
        let ctx = egui::Context::default();
        let mut seed = 9_007_199_254_740_999_u64;
        let mut rect = egui::Rect::NOTHING;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rect = ui.add(DragValue::new(&mut seed)).rect;
            });
        });
        let events = vec![
            egui::Event::PointerMoved(rect.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                modifiers: Default::default(),
            },
        ];
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(DragValue::new(&mut seed));
                });
            },
        );
        assert_eq!(seed, 9_007_199_254_741_000);
        let ctx = egui::Context::default();
        let mut width = 1024_u32;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rect = ui
                    .add(DragValue::new(&mut width).range(256..=4096).speed(32))
                    .rect;
            });
        });
        let events = vec![
            egui::Event::PointerMoved(rect.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                modifiers: Default::default(),
            },
        ];
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(DragValue::new(&mut width).range(256..=4096).speed(32));
                });
            },
        );
        assert_eq!(width, 1056);
    }
}
