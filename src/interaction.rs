//! Application shortcuts, focused text commands and settings shared by the UI.
use egui::{Context, Event, Id, Key, Modifiers, Response, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Fullscreen,
}
pub const ACTIONS: [Action; 7] = [
    Action::Undo,
    Action::Redo,
    Action::Copy,
    Action::Cut,
    Action::Paste,
    Action::SelectAll,
    Action::Fullscreen,
];
impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Fullscreen => "Toggle Fullscreen",
        }
    }
    fn default(self) -> Binding {
        Binding {
            key: match self {
                Self::Undo => Key::Z,
                Self::Redo => Key::Y,
                Self::Copy => Key::C,
                Self::Cut => Key::X,
                Self::Paste => Key::V,
                Self::SelectAll => Key::A,
                Self::Fullscreen => Key::F11,
            },
            ctrl: self != Self::Fullscreen,
            alt: false,
            shift: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}
impl Binding {
    fn matches(self, key: Key, m: Modifiers) -> bool {
        key == self.key
            && self.ctrl == m.ctrl
            && self.alt == m.alt
            && self.shift == m.shift
            && !m.mac_cmd
    }
    pub fn label(self) -> String {
        format!(
            "{}{}{}{}",
            if self.ctrl { "Ctrl+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            self.key.name()
        )
    }
    fn valid(self) -> bool {
        let function = self
            .key
            .name()
            .strip_prefix('F')
            .and_then(|s| s.parse::<u32>().ok())
            .is_some();
        (self.ctrl || self.alt || function)
            && !matches!(
                self.key,
                Key::Escape
                    | Key::Enter
                    | Key::Tab
                    | Key::ArrowLeft
                    | Key::ArrowRight
                    | Key::ArrowUp
                    | Key::ArrowDown
                    | Key::Home
                    | Key::End
                    | Key::Backspace
                    | Key::Delete
                    | Key::Insert
                    | Key::Copy
                    | Key::Cut
                    | Key::Paste
            )
    }
}
fn reference_group(ui: &mut Ui, title: &str, rows: &[(&str, &str)]) {
    ui.add_space(12.0);
    ui.strong(title);
    ui.separator();
    egui::Grid::new(("hotkey_reference", title))
        .num_columns(2)
        .striped(true)
        .max_col_width((ui.available_width() - 16.0).max(120.0) / 2.0)
        .spacing(egui::vec2(16.0, 6.0))
        .show(ui, |ui| {
            for &(action, control) in rows {
                ui.add(egui::Label::new(action).wrap());
                ui.add(egui::Label::new(egui::RichText::new(control).code()).wrap());
                ui.end_row();
            }
        });
}

pub struct Settings {
    pub automatic_updates: bool,
    pub bindings: [Binding; 7],
    capture: Option<usize>,
    candidate: Option<Binding>,
    pub message: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            automatic_updates: true,
            bindings: ACTIONS.map(Action::default),
            capture: None,
            candidate: None,
            message: String::new(),
        }
    }
}
impl Settings {
    pub fn load() -> Self {
        let value = crate::profile::settings_root()
            .and_then(|p| std::fs::read(p.join("interaction.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .unwrap_or_default();
        Self::decode(value)
    }
    fn decode(value: serde_json::Value) -> Self {
        let mut result = Self::default();
        result.automatic_updates = value["automatic_updates"].as_bool().unwrap_or(true);
        if let Some(rows) = value["bindings"].as_array().filter(|rows| rows.len() == 7) {
            let parsed: Option<Vec<Binding>> = rows
                .iter()
                .map(|v| {
                    Some(Binding {
                        key: Key::from_name(v["key"].as_str()?)?,
                        ctrl: v["ctrl"].as_bool()?,
                        alt: v["alt"].as_bool()?,
                        shift: v["shift"].as_bool()?,
                    })
                })
                .collect();
            if let Some(parsed) = parsed.filter(|p| {
                p.iter()
                    .enumerate()
                    .all(|(i, b)| b.valid() && !p[..i].contains(b))
            }) {
                result.bindings = parsed.try_into().unwrap();
            }
        }
        result
    }
    #[cfg(test)]
    pub fn save(&mut self) {}
    #[cfg(not(test))]
    pub fn save(&mut self) {
        let result = (|| -> Result<(), String> {
            let root = crate::profile::settings_root().ok_or("APPDATA unavailable")?;
            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            let value = serde_json::json!({"automatic_updates":self.automatic_updates,"bindings":self.bindings.iter().map(|b|serde_json::json!({"key":b.key.name(),"ctrl":b.ctrl,"alt":b.alt,"shift":b.shift})).collect::<Vec<_>>()});
            let temp = root.join("interaction.json.new");
            std::fs::write(&temp, serde_json::to_vec_pretty(&value).unwrap())
                .map_err(|e| e.to_string())?;
            std::fs::rename(temp, root.join("interaction.json")).map_err(|e| e.to_string())
        })();
        if let Err(e) = result {
            self.message = format!("Cannot save settings: {e}");
        }
    }
    pub fn cancel_capture(&mut self) {
        self.capture = None;
        self.candidate = None;
    }
    fn assign(&mut self, index: usize, binding: Binding) {
        if !binding.valid() {
            self.message =
                "Choose Ctrl/Alt plus a key, or a function key. Navigation keys are reserved."
                    .into();
        } else if self
            .bindings
            .iter()
            .enumerate()
            .any(|(i, b)| i != index && *b == binding)
        {
            self.message = "That shortcut is already assigned. Choose another combination.".into();
        } else {
            self.bindings[index] = binding;
            self.cancel_capture();
            self.message = "Shortcut saved".into();
            self.save();
        }
    }
    pub fn shortcut(&self, action: Action) -> String {
        self.bindings[action as usize].label()
    }
    pub fn updates_ui(&mut self, ui: &mut Ui) {
        ui.heading("Updates");
        ui.separator();
        if ui
            .checkbox(
                &mut self.automatic_updates,
                "Check for updates automatically at startup",
            )
            .changed()
        {
            self.save();
        }
        if crate::profile::is_dev() {
            ui.small("Automatic checks are disabled in PixelForge Dev, which updates from source.");
        }
    }
    pub fn hotkeys_ui(&mut self, ui: &mut Ui) {
        ui.heading("Hotkeys");
        ui.separator();
        for (index, action) in ACTIONS.into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(action.label());
                let label = if self.capture == Some(index) {
                    "Press a shortcut…".into()
                } else {
                    self.bindings[index].label()
                };
                if ui.button(label).clicked() {
                    self.capture = Some(index);
                    self.candidate = Some(self.bindings[index]);
                    self.message =
                        "Press a shortcut, or choose its parts below. Esc cancels.".into();
                }
            });
        }
        if let Some(index) = self.capture {
            let mut binding = self.candidate.unwrap_or(self.bindings[index]);
            ui.horizontal(|ui| {
                ui.checkbox(&mut binding.ctrl, "Ctrl");
                ui.checkbox(&mut binding.alt, "Alt");
                ui.checkbox(&mut binding.shift, "Shift");
                egui::ComboBox::from_id_salt("custom_shortcut_key")
                    .selected_text(binding.key.name())
                    .show_ui(ui, |ui| {
                        for &key in Key::ALL {
                            ui.selectable_value(&mut binding.key, key, key.name());
                        }
                    });
            });
            self.candidate = Some(binding);
            ui.horizontal(|ui| {
                if ui.button("Save Shortcut").clicked() {
                    self.assign(index, binding);
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_capture();
                }
            });
        }
        if ui.button("Reset Hotkeys to Defaults").clicked() {
            self.bindings = ACTIONS.map(Action::default);
            self.cancel_capture();
            self.message = "Default hotkeys restored".into();
            self.save();
        }
        ui.small(&self.message);
        reference_group(ui, "Text editing", &[
            ("Select text", "Mouse drag"),
            ("Text context menu", "Right-click"),
            ("Move cursor", "Arrow keys"),
            ("Move by word", "Ctrl+Left / Right"),
            ("Start / end of line", "Home / End"),
            ("Start / end of text", "Ctrl+Home / End"),
            ("Delete character or selection", "Backspace / Delete"),
            ("Delete word", "Ctrl+Backspace / Delete"),
            ("Next / previous field", "Tab / Shift+Tab"),
            ("Confirm entry", "Enter"),
            ("Cancel entry or applicable dialog", "Esc"),
        ]);
        ui.small("Select All uses the customizable shortcut above. Other text selection uses the mouse. Shift+navigation selection and Ctrl+H/W/U/K deletion are disabled.");
        reference_group(ui, "Default text aliases", &[
            ("Redo", "Ctrl+Shift+Z"),
            ("Copy", "Ctrl+Insert"),
            ("Cut", "Shift+Delete"),
            ("Paste", "Shift+Insert"),
        ]);
        ui.small("These aliases apply while the corresponding customizable shortcut uses its default binding.");
        reference_group(ui, "Slider controls", &[
            ("Adjust by 1", "Alt+wheel"),
            ("Adjust by 0.1", "Ctrl+Alt+wheel"),
            ("Adjust by 5", "Shift+Alt+wheel"),
            ("Adjust value", "Click and drag"),
            ("Fine adjustment", "Ctrl+drag"),
            ("Scroll panel", "Wheel without Alt"),
        ]);
        reference_group(ui, "Numeric fields", &[
            ("Type a value", "Click the value"),
            ("Adjust by 1", "Wheel"),
            ("Adjust by 0.1", "Ctrl+wheel"),
            ("Adjust by 5", "Shift+wheel"),
            ("Adjust / fine adjustment", "Drag / Ctrl+drag"),
        ]);
        ui.small("Decimal steps apply to decimal-capable fields. Steps use displayed units and respect limits and dimension rounding.");
        reference_group(ui, "Workspace and windows", &[
            ("Zoom image", "Wheel over image"),
            ("Pan / fit image", "See Mouse Controls tab"),
            ("Reset crop view", "Middle-button double-click"),
            ("Activate saved selection", "Double-click selection"),
            ("Move comparison divider", "Drag with Compare enabled"),
            ("Move window", "Drag unused title-bar space"),
            ("Maximize / restore window", "Double-click title bar"),
            ("Close window", "Alt+F4"),
        ]);
        ui.small("Crop zoom follows the selected crop zoom mode. Fullscreen uses the customizable shortcut above.");
        ui.add_space(12.0);
    }
    /// Translate custom commands to the editor's native events before widgets consume input.
    pub fn begin_frame(&mut self, ctx: &Context) -> Vec<Action> {
        ctx.data_mut(|d| d.insert_temp(Id::new("pf_bindings"), self.bindings));
        let pending = ctx.data_mut(|d| {
            let value = d.get_temp::<Queued>(Id::new("pf_text_command"));
            d.remove::<Queued>(Id::new("pf_text_command"));
            value
        });
        let mut actions = Vec::new();
        let events = ctx.input_mut(|i| std::mem::take(&mut i.events));
        let mut output = Vec::new();
        for event in events {
            let event = if self.capture.is_some() {
                let key = match event {
                    Event::Copy => Some(Key::C),
                    Event::Cut => Some(Key::X),
                    Event::Paste(_) => Some(Key::V),
                    _ => None,
                };
                if let Some(key) = key {
                    Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: ctx.input(|i| i.modifiers),
                    }
                } else {
                    event
                }
            } else {
                event
            };
            if let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = &event
            {
                if let Some(index) = self.capture {
                    if *key == Key::Escape {
                        self.cancel_capture();
                        self.message.clear();
                    } else {
                        let binding = Binding {
                            key: *key,
                            ctrl: modifiers.ctrl,
                            alt: modifiers.alt,
                            shift: modifiers.shift,
                        };
                        self.assign(index, binding);
                    }
                    continue;
                }
                if let Some(index) = self
                    .bindings
                    .iter()
                    .position(|b| b.matches(*key, *modifiers))
                {
                    actions.push(ACTIONS[index]);
                    continue;
                }
                if self.bindings[Action::Redo as usize] == Action::Redo.default()
                    && *key == Key::Z
                    && modifiers.ctrl
                    && modifiers.shift
                    && !modifiers.alt
                {
                    actions.push(Action::Redo);
                    continue;
                }
                // Retire old bindings after reassignment, including native redo and clipboard aliases.
                if ACTIONS
                    .iter()
                    .any(|a| a.default().matches(*key, *modifiers))
                    || (modifiers.ctrl && matches!(key, Key::H | Key::W | Key::U | Key::K))
                    || (modifiers.ctrl && *key == Key::Insert)
                    || (modifiers.shift && matches!(key, Key::Insert | Key::Delete))
                    || (modifiers.ctrl && modifiers.shift && *key == Key::Z)
                {
                    continue;
                }
            }
            // Native Copy/Cut/Paste events arrive instead of Ctrl+C/X/V key events.
            let native = match &event {
                Event::Copy => Some(Action::Copy),
                Event::Cut => Some(Action::Cut),
                Event::Paste(_) => Some(Action::Paste),
                _ => None,
            };
            if let Some(action) = native {
                let mods = ctx.input(|i| i.modifiers);
                let alias = matches!(action, Action::Paste | Action::Cut)
                    && mods.shift
                    && !mods.ctrl
                    && !mods.alt
                    && self.bindings[action as usize] == action.default();
                if self.bindings[action as usize].matches(action.default().key, mods) || alias {
                    output.push(event);
                } else if let Some(index) = self
                    .bindings
                    .iter()
                    .position(|b| b.matches(action.default().key, mods))
                {
                    actions.push(ACTIONS[index]);
                }
                continue;
            }
            let mut event = event;
            if let Event::Key { key, modifiers, .. } = &mut event {
                if matches!(
                    key,
                    Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::ArrowUp
                        | Key::ArrowDown
                        | Key::Home
                        | Key::End
                ) {
                    modifiers.shift = false;
                }
            }
            output.push(event);
        }
        if let Some(queued) = pending {
            inject(ctx, queued, &mut output);
        }
        ctx.input_mut(|i| i.events = output);
        actions
    }
}

#[derive(Clone)]
struct Target {
    id: Id,
    selected: bool,
    undo: bool,
    redo: bool,
    frame: u64,
}
#[derive(Clone)]
struct Queued {
    target: Option<Id>,
    action: Action,
    readonly: Option<egui::text_selection::LabelSelectionState>,
}
fn target(ctx: &Context) -> Option<Target> {
    ctx.data(|d| d.get_temp::<Target>(Id::new("pf_text_target")))
        .filter(|t| ctx.cumulative_frame_nr().saturating_sub(t.frame) <= 1)
}
pub fn has_text_target(ctx: &Context) -> bool {
    target(ctx).is_some() && readonly(ctx).is_none()
}
#[cfg(test)]
thread_local! {static TEST_CLIPBOARD:std::cell::RefCell<Option<String>>=const {std::cell::RefCell::new(None)};}
#[cfg(test)]
fn clipboard() -> Option<String> {
    TEST_CLIPBOARD
        .with(|c| c.borrow().clone())
        .filter(|s| !s.is_empty())
}
#[cfg(not(test))]
fn clipboard() -> Option<String> {
    arboard::Clipboard::new()
        .ok()?
        .get_text()
        .ok()
        .filter(|s| !s.is_empty())
}
fn label_selection(ctx: &Context) -> bool {
    egui::text_selection::LabelSelectionState::load(ctx).has_selection()
}
#[derive(Clone)]
struct Readonly {
    selection: egui::text_selection::LabelSelectionState,
    frame: u64,
}
fn readonly(ctx: &Context) -> Option<Readonly> {
    ctx.data(|d| d.get_temp::<Readonly>(Id::new("pf_readonly")))
        .filter(|t| ctx.cumulative_frame_nr().saturating_sub(t.frame) <= 1)
}
pub fn keep_selection(ctx: &Context) {
    if let Some(mut value) = readonly(ctx) {
        value.frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Id::new("pf_readonly"), value));
    }
}
fn record_label(ctx: &Context) {
    let selection = egui::text_selection::LabelSelectionState::load(ctx);
    if selection.has_selection() {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Id::new("pf_readonly"), Readonly { selection, frame }));
    }
}
pub fn enabled(ctx: &Context, action: Action) -> bool {
    let target = target(ctx).filter(|_| readonly(ctx).is_none());
    match action {
        Action::Copy => target.is_some_and(|t| t.selected) || readonly(ctx).is_some(),
        Action::Cut => target.is_some_and(|t| t.selected),
        Action::Paste => target.is_some() && clipboard().is_some(),
        Action::Undo => target.is_some_and(|t| t.undo),
        Action::Redo => target.is_some_and(|t| t.redo),
        Action::SelectAll => target.is_some(),
        Action::Fullscreen => true,
    }
}
pub fn queue(ctx: &Context, action: Action) {
    let id = target(ctx)
        .filter(|_| readonly(ctx).is_none())
        .map(|t| t.id);
    let selection = if id.is_none() {
        readonly(ctx).map(|r| r.selection)
    } else {
        None
    };
    ctx.data_mut(|d| {
        d.insert_temp(
            Id::new("pf_text_command"),
            Queued {
                target: id,
                action,
                readonly: selection,
            },
        )
    });
    ctx.request_repaint();
}
fn inject(ctx: &Context, queued: Queued, events: &mut Vec<Event>) {
    if let Some(selection) = queued.readonly {
        selection.store(ctx);
    }
    if let Some(id) = queued.target {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    let key = |key, shift| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            shift,
            ..Default::default()
        },
    };
    match queued.action {
        Action::Copy => events.push(Event::Copy),
        Action::Cut => events.push(Event::Cut),
        Action::Paste => {
            if let Some(text) = clipboard() {
                events.push(Event::Paste(text));
            }
        }
        Action::Undo => events.push(key(Key::Z, false)),
        Action::Redo => events.push(key(Key::Y, false)),
        Action::SelectAll => events.push(key(Key::A, false)),
        Action::Fullscreen => {}
    }
}
fn record(ui: &Ui, id: Id, text: &str, state: &egui::text_edit::TextEditState) {
    let active = ui.memory(|m| m.has_focus(id)) || target(ui.ctx()).is_some_and(|t| t.id == id);
    if active {
        if ui.memory(|m| m.has_focus(id)) {
            ui.ctx()
                .data_mut(|d| d.remove::<Readonly>(Id::new("pf_readonly")));
        }
        let range = state.cursor.char_range().unwrap_or_default();
        let current = (range, text.to_owned());
        let undoer = state.undoer();
        let frame = ui.ctx().cumulative_frame_nr();
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                Id::new("pf_text_target"),
                Target {
                    id,
                    selected: !range.is_empty(),
                    undo: undoer.has_undo(&current),
                    redo: undoer.has_redo(&current),
                    frame,
                },
            )
        });
    }
}
// egui 0.32 moves the insertion point on any pointer press, including a
// secondary press. Restore the selection before opening the editor menu.
pub fn secondary_selection(ui: &Ui, id: Id) -> Option<egui::text::CCursorRange> {
    if !ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Secondary)) {
        return None;
    }
    egui::text_edit::TextEditState::load(ui.ctx(), id)?
        .cursor
        .char_range()
        .filter(|range| !range.is_empty())
}

pub fn restore_secondary_selection(
    ui: &Ui,
    output: &mut egui::text_edit::TextEditOutput,
    selection: Option<egui::text::CCursorRange>,
) {
    if let Some(selection) = selection.filter(|_| output.response.contains_pointer()) {
        output.state.cursor.set_char_range(Some(selection));
        output.state.clone().store(ui.ctx(), output.response.id);
        output.response.request_focus();
        ui.ctx().request_repaint();
    }
}

pub fn text_menu(
    ui: &Ui,
    response: &Response,
    text: &str,
    state: &egui::text_edit::TextEditState,
    extra: impl FnOnce(&mut Ui),
) {
    record(ui, response.id, text, state);
    response.context_menu(|ui| {
        let frame = ui.ctx().cumulative_frame_nr();
        ui.ctx().data_mut(|d| {
            d.remove::<Readonly>(Id::new("pf_readonly"));
            d.insert_temp(
                Id::new("pf_text_target"),
                Target {
                    id: response.id,
                    selected: state.cursor.char_range().is_some_and(|r| !r.is_empty()),
                    undo: false,
                    redo: false,
                    frame,
                },
            )
        });
        for action in [Action::Cut, Action::Copy, Action::Paste] {
            let bindings = ui
                .ctx()
                .data(|d| d.get_temp::<[Binding; 7]>(Id::new("pf_bindings")))
                .unwrap_or(ACTIONS.map(Action::default));
            if ui
                .add_enabled(
                    enabled(ui.ctx(), action),
                    egui::Button::new(action.label())
                        .shortcut_text(bindings[action as usize].label()),
                )
                .clicked()
            {
                queue(ui.ctx(), action);
                ui.close();
            }
        }
        extra(ui);
    });
}
pub fn singleline(ui: &mut Ui, text: &mut String) -> Response {
    let id = ui.next_auto_id();
    let selection = secondary_selection(ui, id);
    let mut output = egui::TextEdit::singleline(&mut *text).id(id).show(ui);
    restore_secondary_selection(ui, &mut output, selection);
    text_menu(ui, &output.response, text, &output.state, |_| {});
    output.response
}
pub fn numeric_text(ui: &Ui, response: &Response) {
    if let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), response.id) {
        let text = ui
            .data(|d| d.get_temp::<String>(response.id))
            .unwrap_or_default();
        text_menu(ui, response, &text, &state, |_| {});
    }
}
pub fn label(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    let response = ui.add(egui::Label::new(text).selectable(true));
    readonly_menu(ui, &response);
    response
}

pub fn readonly_menu(ui: &Ui, response: &Response) {
    record_label(ui.ctx());
    response.context_menu(|ui| {
        keep_selection(ui.ctx());
        for action in [Action::Cut, Action::Copy, Action::Paste] {
            let can = action == Action::Copy && readonly(ui.ctx()).is_some();
            let bindings = ui
                .ctx()
                .data(|d| d.get_temp::<[Binding; 7]>(Id::new("pf_bindings")))
                .unwrap_or(ACTIONS.map(Action::default));
            if ui
                .add_enabled(
                    can,
                    egui::Button::new(action.label())
                        .shortcut_text(bindings[action as usize].label()),
                )
                .clicked()
            {
                let selection = readonly(ui.ctx()).map(|r| r.selection);
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        Id::new("pf_text_command"),
                        Queued {
                            target: None,
                            action: Action::Copy,
                            readonly: selection,
                        },
                    )
                });
                ui.ctx().request_repaint();
                ui.close();
            }
        }
    });
}
pub fn label_widget(ui: &mut Ui, label: egui::Label) -> Response {
    let response = ui.add(label.selectable(true));
    readonly_menu(ui, &response);
    response
}
pub fn small(ui: &mut Ui, text: impl Into<egui::RichText>) -> Response {
    label(ui, text.into().small())
}
pub fn strong(ui: &mut Ui, text: impl Into<egui::RichText>) -> Response {
    label(ui, text.into().strong())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            modifiers,
            pressed: true,
            physical_key: None,
            repeat: false,
        }
    }
    fn ctrl() -> Modifiers {
        Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        }
    }
    fn text_frame(
        ctx: &Context,
        settings: &mut Settings,
        text: &mut String,
        events: Vec<Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                time: Some(ctx.cumulative_frame_nr() as f64),
                events,
                modifiers: ctrl(),
                ..Default::default()
            },
            |ctx| {
                for action in settings.begin_frame(ctx) {
                    if enabled(ctx, action) {
                        queue(ctx, action);
                    }
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = egui::TextEdit::singleline(&mut *text)
                        .id(Id::new("test_text"))
                        .show(ui);
                    text_menu(ui, &output.response, text, &output.state, |_| {});
                });
            },
        )
    }
    #[test]
    fn text_menu_commands_keep_selection_and_route_to_the_original_field() {
        let ctx = Context::default();
        let mut settings = Settings::default();
        let mut text = "hello world".to_owned();
        ctx.memory_mut(|m| m.request_focus(Id::new("test_text")));
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        queue(&ctx, Action::SelectAll);
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert!(enabled(&ctx, Action::Copy));
        assert!(enabled(&ctx, Action::Cut));
        queue(&ctx, Action::Copy);
        ctx.memory_mut(|m| m.surrender_focus(Id::new("test_text")));
        let output = text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c,egui::OutputCommand::CopyText(t) if t=="hello world"))
        );
        queue(&ctx, Action::Cut);
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert_eq!(text, "");
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert!(enabled(&ctx, Action::Undo));
        queue(&ctx, Action::Undo);
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert_eq!(text, "hello world");
        queue(&ctx, Action::Redo);
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert_eq!(text, "");
        TEST_CLIPBOARD.with(|c| *c.borrow_mut() = Some("pasted".into()));
        assert!(enabled(&ctx, Action::Paste));
        queue(&ctx, Action::Paste);
        text_frame(&ctx, &mut settings, &mut text, vec![]);
        assert_eq!(text, "pasted");
        TEST_CLIPBOARD.with(|c| *c.borrow_mut() = None);
    }
    #[test]
    fn remapped_keys_replace_defaults_and_removed_text_actions_do_not_leak() {
        let ctx = Context::default();
        let mut settings = Settings::default();
        settings.bindings[Action::Copy as usize] = Binding {
            key: Key::Q,
            ..Action::Copy.default()
        };
        settings.bindings[Action::Fullscreen as usize] = Binding {
            key: Key::F10,
            ..Action::Fullscreen.default()
        };
        let events = vec![
            key(Key::Q, ctrl()),
            key(Key::C, ctrl()),
            Event::Copy,
            key(Key::H, ctrl()),
            key(Key::W, ctrl()),
            key(Key::U, ctrl()),
            key(Key::K, ctrl()),
            key(Key::A, ctrl()),
            key(Key::F11, Modifiers::NONE),
            key(Key::F10, Modifiers::NONE),
            key(
                Key::ArrowLeft,
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
            ),
        ];
        let _=ctx.run(egui::RawInput {events,modifiers:ctrl(),..Default::default()},|ctx| {
            assert_eq!(settings.begin_frame(ctx),vec![Action::Copy,Action::SelectAll,Action::Fullscreen]);
            ctx.input(|i| {
                assert!(!i.events.iter().any(|e|matches!(e,Event::Copy)));
                assert!(i.events.iter().any(|e|matches!(e,Event::Key {key:Key::ArrowLeft,modifiers,..} if !modifiers.shift)));
            });
        });
    }
    #[test]
    fn duplicate_capture_is_rejected_and_cancel_does_not_change_bindings() {
        let ctx = Context::default();
        let mut settings = Settings::default();
        settings.capture = Some(Action::Copy as usize);
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key(Key::Y, ctrl())],
                ..Default::default()
            },
            |ctx| {
                assert!(settings.begin_frame(ctx).is_empty());
            },
        );
        assert!(settings.message.contains("already assigned"));
        assert_eq!(
            settings.bindings[Action::Copy as usize],
            Action::Copy.default()
        );
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key(Key::Escape, Modifiers::NONE)],
                ..Default::default()
            },
            |ctx| {
                settings.begin_frame(ctx);
            },
        );
        assert!(settings.capture.is_none());
        let value = serde_json::json!({"automatic_updates":false,"bindings":ACTIONS.map(|a| {let b=a.default();serde_json::json!({"key":b.key.name(),"ctrl":b.ctrl,"shift":b.shift,"alt":b.alt})})});
        let parsed = Settings::decode(value);
        assert!(!parsed.automatic_updates);
        assert_eq!(parsed.bindings, ACTIONS.map(Action::default));
    }
    #[test]
    fn readonly_selection_survives_a_menu_and_cannot_be_cut_or_pasted() {
        let ctx = Context::default();
        let mut settings = Settings::default();
        let mut rect = egui::Rect::NOTHING;
        TEST_CLIPBOARD.with(|c| *c.borrow_mut() = Some("must not paste".into()));
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    settings.begin_frame(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect =
                            label(ui, "Readonly text example for copying a selected portion").rect;
                    });
                },
            )
        };
        frame(vec![]);
        // Drop the closure's mutable borrow while choosing screen coordinates.
        let first = egui::pos2(9.0, 16.0);
        let last = egui::pos2(90.0, 16.0);
        let button = |pos, pressed| Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        frame(vec![Event::PointerMoved(first), button(first, true)]);
        frame(vec![Event::PointerMoved(last)]);
        frame(vec![button(last, false)]);
        assert!(enabled(&ctx, Action::Copy));
        assert!(!enabled(&ctx, Action::Cut));
        assert!(!enabled(&ctx, Action::Paste));
        let mut current = egui::text_selection::LabelSelectionState::load(&ctx);
        current.clear_selection();
        current.store(&ctx);
        keep_selection(&ctx);
        queue(&ctx, Action::Copy);
        let output = frame(vec![]);
        assert!(output.platform_output.commands.iter().any(
            |c| matches!(c,egui::OutputCommand::CopyText(text) if !text.is_empty() && text.len()<52)
        ));
        TEST_CLIPBOARD.with(|c| *c.borrow_mut() = None);
    }
}
