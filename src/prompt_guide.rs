use crate::{
    ai_assist, ai_generation,
    model_installer::MODELS,
    quick_assist::{self, Profile},
    spelling::{PromptSpelling, SCRATCHPAD_FIELD},
};

const QUALITY: &[&str] = &["score_9", "score_8_up", "score_7_up"];
const COMMON_TAGS: &[(&str, &[&str])] = &[
    ("Quality", QUALITY),
    ("Subject", &["solo", "1girl", "1boy", "smile"]),
    (
        "Clothing",
        &[
            "shirt",
            "t-shirt",
            "tank_top",
            "sweater",
            "hoodie",
            "jacket",
            "coat",
            "pants",
            "jeans",
            "shorts",
            "skirt",
            "dress",
            "suit",
            "socks",
            "sneakers",
            "boots",
            "high_heels",
            "hat",
            "scarf",
            "gloves",
            "necktie",
            "belt",
        ],
    ),
    (
        "Pose",
        &["looking_at_viewer", "looking_away", "standing", "sitting"],
    ),
    (
        "Framing",
        &[
            "portrait",
            "close-up",
            "upper_body",
            "full_body",
            "from_side",
        ],
    ),
    (
        "Setting",
        &["indoors", "outdoors", "night", "simple_background"],
    ),
    ("Other", &[]),
];

type TagCategories = std::collections::BTreeMap<String, Vec<String>>;
type SavedPrompts = std::collections::BTreeMap<String, String>;
type GuideSizes = std::collections::BTreeMap<String, [f32; 2]>;

const LAYOUT_KEY: &str = "prompt-guide-layout";
const SCRATCHPAD_ROWS: usize = 6;

fn profile_file(name: &str) -> Option<std::path::PathBuf> {
    Some(crate::profile::settings_root()?.join(name))
}

fn write_profile_file(name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = profile_file(name)
        .ok_or_else(|| std::io::Error::other("PixelForge profile unavailable"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.new");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}

#[derive(Default)]
pub struct UserTags {
    saved: TagCategories,
    category: usize,
    draft: String,
    error: String,
}

impl UserTags {
    pub fn load() -> Self {
        let saved = profile_file("prompt-tags.json")
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| decode_tags(&bytes))
            .unwrap_or_default();
        Self {
            saved,
            ..Self::default()
        }
    }

    fn save(&mut self, tags: TagCategories) -> bool {
        let result = serde_json::to_vec(&tags)
            .map_err(std::io::Error::from)
            .and_then(|bytes| write_profile_file("prompt-tags.json", &bytes));
        match result {
            Ok(()) => {
                self.saved = tags;
                self.error.clear();
                true
            }
            Err(error) => {
                self.error = format!("Could not save user tags. {error}");
                false
            }
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, prompt: &mut String) {
        let mut remove = None;
        for (heading, tags) in COMMON_TAGS {
            ui.strong(*heading);
            ui.horizontal_wrapped(|ui| {
                for tag in *tags {
                    tag_button(ui, prompt, tag);
                }
                if let Some(saved) = self.saved.get(*heading) {
                    for (index, tag) in saved.iter().enumerate() {
                        ui.push_id((heading, index), |ui| {
                            let response = tag_button(ui, prompt, tag);
                            response.context_menu(|ui| {
                                if ui.button("Remove").clicked() {
                                    remove = Some((*heading, index));
                                    ui.close();
                                }
                            });
                        });
                    }
                }
            });
        }
        if let Some((heading, index)) = remove {
            let mut tags = self.saved.clone();
            if let Some(saved) = tags.get_mut(heading) {
                saved.remove(index);
                self.save(tags);
            }
        }
        ui.separator();
        ui.strong("User Tags");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("user_tag_category")
                .selected_text(COMMON_TAGS[self.category].0)
                .show_ui(ui, |ui| {
                    for (index, (heading, _)) in COMMON_TAGS.iter().enumerate() {
                        ui.selectable_value(&mut self.category, index, *heading);
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.draft)
                    .hint_text("Enter a tag")
                    .desired_width(180.0),
            );
            if ui.button("Save tag").clicked() {
                let (category, defaults) = COMMON_TAGS[self.category];
                let tag = self.draft.trim();
                if tag.is_empty() || tag.contains([',', '\r', '\n']) {
                    self.error = "Enter one tag at a time.".into();
                } else if defaults.iter().any(|default| same_tag(default, tag))
                    || self
                        .saved
                        .get(category)
                        .is_some_and(|saved| saved.iter().any(|saved| same_tag(saved, tag)))
                {
                    self.error = "That tag is already saved.".into();
                } else {
                    let mut tags = self.saved.clone();
                    tags.entry(category.to_owned())
                        .or_default()
                        .push(tag.to_owned());
                    if self.save(tags) {
                        self.draft.clear();
                    }
                }
            }
        });
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
    }
}

fn decode_tags(bytes: &[u8]) -> Option<TagCategories> {
    serde_json::from_slice(bytes).ok().or_else(|| {
        let legacy: Vec<String> = serde_json::from_slice(bytes).ok()?;
        Some(TagCategories::from([("Other".to_owned(), legacy)]))
    })
}

fn same_tag(left: &str, right: &str) -> bool {
    left.trim()
        .replace('_', " ")
        .eq_ignore_ascii_case(&right.trim().replace('_', " "))
}

fn insert_tag(prompt: &mut String, tag: &str) {
    if prompt.split(',').any(|part| same_tag(part, tag)) {
        return;
    }
    let existing = prompt.trim().trim_end_matches(',').trim_end();
    if existing.is_empty() {
        *prompt = tag.to_owned();
    } else {
        *prompt = format!("{existing}, {tag}");
    }
}

fn tag_button(ui: &mut egui::Ui, prompt: &mut String, tag: &str) -> egui::Response {
    let response = ui.button(tag);
    if response.clicked() {
        insert_tag(prompt, tag);
    }
    response
}

struct Guide {
    structure: &'static str,
    example: &'static str,
    negative: &'static str,
    danbooru: bool,
    assist_profile: Profile,
}

const GENERAL: Guide = Guide {
    structure: "General starting point: describe the subject, setting, composition, lighting, and desired medium. Check the model author's guidance for special tags or syntax.",
    example: "A small lighthouse on a rocky coast at dawn, wide composition, soft mist, watercolor illustration.",
    negative: "Check whether your checkpoint supports negative prompts before using one.",
    danbooru: false,
    assist_profile: Profile::General,
};

fn model_guide(keyword: &str) -> Option<Guide> {
    let (structure, example, negative) = match keyword {
        "cyberrealisticpony" => (
            "Start with Pony score tags, then add comma-separated Danbooru-style subject, pose, and framing tags. Add setting and lighting details after those. For illustration, name the medium near the start of the subject description. This checkpoint may still favor a photographic look.",
            "score_9, score_8_up, score_7_up, solo, 1girl, adult woman, looking at viewer, upper body, green jacket, outdoors, soft morning light, realistic",
            "For a photographic look, try excluding illustration or cartoon if those styles appear.",
        ),
        "cyberrealisticxl" => (
            "Describe the person and framing, then add a setting, light direction, and photographic details such as depth of field.",
            "Waist-up photograph of a carpenter in a woodshop, warm window light from the left, natural skin texture, softly blurred tools in the background.",
            "Add a short exclusion if a specific artifact keeps appearing.",
        ),
        "realvis" => (
            "Start with a photographic subject and setting, then add the composition, color palette, and light.",
            "Landscape photograph of a quiet mountain lake, low viewpoint, cool muted colors, pale mist above the water, overcast morning light.",
            "For portraits, the author suggests targeting anatomy and facial asymmetry if they appear.",
        ),
        "juggernaut" => (
            "Lead with the shot type and subject, then describe the scene, lighting, and depth of field.",
            "Cinematic wide shot of a vintage train crossing a stone bridge, evening fog in the valley, warm light through the carriage windows.",
            "RunDiffusion suggests starting with an empty negative prompt. Turn off automatic negatives to try this, then add specific exclusions as needed.",
        ),
        "sd_xl_base" => (
            "Name the subject and medium first, then add the setting, composition, palette, and lighting.",
            "Gouache illustration of a red fox in a snowy pine forest, centered composition, muted blue shadows, warm sunset highlights.",
            "Try short, specific exclusions if the output repeatedly includes unwanted details.",
        ),
        "cyberrealistic_zit" => (
            "Describe the scene in natural language. Include the subject, surroundings, composition, and how the light falls.",
            "A ceramic cup sits on a worn kitchen table beside a rain-streaked window. Cool daylight catches the glaze in a close-up photograph.",
            "",
        ),
        "fluxklein4b" => (
            "Describe the image in natural language. Put the subject and action first, then specify the setting, framing, and style.",
            "A cyclist in a yellow raincoat rides through a quiet city intersection at dusk. Frame the scene from street level with reflections across the wet pavement.",
            "",
        ),
        _ => return None,
    };
    Some(Guide {
        structure,
        example,
        negative,
        danbooru: keyword == "cyberrealisticpony",
        assist_profile: match keyword {
            "cyberrealisticpony" => Profile::Pony,
            "cyberrealistic_zit" | "fluxklein4b" => Profile::NaturalLanguage,
            _ => Profile::General,
        },
    })
}

fn guide_for(checkpoint: &str) -> (Guide, Option<&'static str>) {
    let filename = checkpoint.rsplit(['/', '\\']).next().unwrap_or_default();
    for model in MODELS.iter().filter(|model| model.generation) {
        if (model.name == checkpoint
            || model.files.iter().any(|file| {
                file.directory == "checkpoints" && file.file.eq_ignore_ascii_case(filename)
            }))
            && let Some(guide) = model_guide(model.keyword)
        {
            return (guide, Some(model.page));
        }
    }
    (GENERAL, None)
}

#[derive(Default)]
pub struct PromptGuide {
    pub open: bool,
    scratchpad: Scratchpad,
    assist: Option<AssistPreview>,
    local_assist: LocalAssist,
    user_tags: UserTags,
    anchor: Option<[f32; 2]>,
    sizes: GuideSizes,
}

impl PromptGuide {
    pub fn load() -> Self {
        Self {
            scratchpad: Scratchpad::load(),
            user_tags: UserTags::load(),
            ..Self::default()
        }
    }

    pub fn restore_layout(&mut self, storage: &dyn eframe::Storage) {
        if let Some((anchor, sizes)) =
            eframe::get_value::<(Option<[f32; 2]>, GuideSizes)>(storage, LAYOUT_KEY)
        {
            self.anchor = anchor.filter(|position| position.iter().all(|v| v.is_finite()));
            self.sizes = sizes
                .into_iter()
                .filter(|(_, size)| size.iter().all(|v| v.is_finite() && *v > 0.0))
                .collect();
        }
    }

    pub fn save_layout(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, LAYOUT_KEY, &(self.anchor, &self.sizes));
    }

    pub(crate) fn show_ollama_dependency(&mut self, ui: &mut egui::Ui, checkpoint: &str) {
        let local = &mut self.local_assist;
        local.poll(checkpoint, &self.scratchpad.draft, &mut self.assist);
        crate::theme::section(ui, "Ollama · Optional for AI Assist");
        ui.label("Runs local text models for scratchpad rewrites. Download text models in Model Library after setup.");
        ui.hyperlink_to("Ollama for Windows", "https://ollama.com/download/windows");
        #[cfg(windows)]
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    local.pending.is_none(),
                    egui::Button::new("Download Ollama Installer"),
                )
                .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title("Save the Ollama installer")
                    .set_file_name("OllamaSetup.exe")
                    .add_filter("Windows installer", &["exe"])
                    .save_file()
            {
                let progress = std::sync::Arc::new(InstallerProgress::default());
                local.installer_progress = Some(progress.clone());
                local.start(ui.ctx(), "Downloading the Ollama installer…", move || {
                    AssistReply::Installer(ai_assist::download_ollama_installer(
                        path,
                        |received, total| {
                            *progress.lock().unwrap() = (received, total);
                        },
                    ))
                });
            }
            if ui
                .add_enabled(local.pending.is_none(), egui::Button::new("Open Installer"))
                .on_hover_text("Open the downloaded installer or select a saved copy.")
                .clicked()
                && let Some(path) = local
                    .installer
                    .as_ref()
                    .filter(|path| path.is_file())
                    .cloned()
                    .or_else(|| {
                        rfd::FileDialog::new()
                            .set_title("Select the downloaded Ollama installer")
                            .add_filter("Windows installer", &["exe"])
                            .pick_file()
                    })
            {
                local.start(ui.ctx(), "Verifying the Windows installer…", move || {
                    AssistReply::InstallerOpened(ai_assist::open_ollama_installer(&path))
                });
            }
        });
        let idle = local.pending.is_none();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(idle, egui::Button::new("Check Ollama"))
                .clicked()
            {
                local.start(ui.ctx(), "Checking Ollama…", || {
                    AssistReply::Models(ai_assist::models())
                });
            }
            if local.connected {
                ui.label("Ollama is running.");
            }
        });
        local.show_status(ui);
    }

    pub(crate) fn show_ollama_library(&mut self, ui: &mut egui::Ui, checkpoint: &str) -> bool {
        let local = &mut self.local_assist;
        local.poll(checkpoint, &self.scratchpad.draft, &mut self.assist);
        ui.strong("Ollama text models · AI Assist");
        ui.label("Optional local text models for rewriting Prompt Guide scratchpad drafts.");
        ui.heading(egui::RichText::new("Qwen2.5 3B").size(23.0).strong());
        ui.label("Requires Ollama. CPU: available through Ollama. NVIDIA, AMD and Intel acceleration depends on Ollama’s hardware support.");
        ui.label("About 1.9 GB on disk. Runtime memory depends on context size.");
        let open_dependencies = !local.connected && ui.button("Open Ollama Setup").clicked();
        let idle = local.pending.is_none();
        let starter_installed = local
            .models
            .iter()
            .any(|model| model == ai_assist::STARTER_MODEL);
        if ui
            .add_enabled(
                idle && local.connected && !starter_installed,
                egui::Button::new("Download Qwen2.5 3B (1.9 GB)"),
            )
            .clicked()
        {
            let progress =
                std::sync::Arc::new(std::sync::Mutex::new(ai_assist::ModelProgress::default()));
            local.model_progress = Some(progress.clone());
            local.start(
                ui.ctx(),
                "Downloading Qwen2.5 3B through Ollama…",
                move || {
                    AssistReply::Download(ai_assist::download_starter_model(|update| {
                        *progress.lock().unwrap() = update;
                    }))
                },
            );
        }
        local.show_status(ui);
        if local.connected && !local.models.is_empty() {
            ui.label("Installed local models");
            for model in &local.models {
                ui.heading(egui::RichText::new(model).size(23.0).strong());
            }
        }
        open_dependencies
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        checkpoint: &str,
        prompt: &mut String,
        spelling: &mut PromptSpelling,
    ) {
        if self.scratchpad.model.as_deref() != Some(checkpoint) {
            self.assist = None;
        }
        self.scratchpad.select_model(checkpoint);
        let [width, height] = self
            .sizes
            .get(checkpoint)
            .copied()
            .unwrap_or([420.0, 440.0]);
        let mut window = egui::Window::new("Prompt Guide")
            .id(egui::Id::new(("prompt-guide", checkpoint)))
            .open(&mut self.open)
            .collapsible(false)
            .pivot(egui::Align2::LEFT_TOP)
            .default_size(egui::vec2(width, height));
        if let Some([x, y]) = self.anchor {
            window = window.current_pos(egui::pos2(x, y));
        }
        if let Some(output) = window.show(ctx, |ui| {
            let size = ui.available_size();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let guide = guide_for(checkpoint).0;
                    self.scratchpad.show(
                        ui,
                        prompt,
                        guide.danbooru.then_some(&self.user_tags),
                        spelling,
                    );
                    if self.scratchpad.open {
                        show_assist(
                            ui,
                            &mut self.assist,
                            &mut self.local_assist,
                            &mut self.scratchpad.draft,
                            checkpoint,
                            guide.assist_profile,
                        );
                    } else {
                        self.assist = None;
                    }
                    ui.separator();
                    show_guide(
                        ui,
                        checkpoint,
                        self.scratchpad.tag_target(prompt),
                        &mut self.user_tags,
                    );
                });
            size
        }) && let Some(size) = output.inner
        {
            let position = output.response.rect.min;
            self.anchor = Some([position.x, position.y]);
            self.sizes.insert(checkpoint.to_owned(), [size.x, size.y]);
        }
    }
}

struct AssistPreview {
    source: String,
    suggestion: String,
}

#[cfg(windows)]
type InstallerProgress = std::sync::Mutex<(u64, Option<u64>)>;

#[derive(Default)]
struct LocalAssist {
    open: bool,
    models: Vec<String>,
    selected: String,
    connected: bool,
    #[cfg(windows)]
    installer: Option<std::path::PathBuf>,
    #[cfg(windows)]
    installer_progress: Option<std::sync::Arc<InstallerProgress>>,
    activity: &'static str,
    rewrite_progress: Option<(std::time::Instant, std::sync::Arc<std::sync::Mutex<String>>)>,
    model_progress: Option<std::sync::Arc<std::sync::Mutex<ai_assist::ModelProgress>>>,
    pending: Option<std::sync::mpsc::Receiver<AssistReply>>,
    message: String,
}

enum AssistReply {
    Models(Result<Vec<String>, String>),
    Download(Result<Vec<String>, String>),
    #[cfg(windows)]
    Installer(Result<std::path::PathBuf, String>),
    #[cfg(windows)]
    InstallerOpened(Result<(), String>),
    Rewrite {
        checkpoint: String,
        source: String,
        result: Result<String, String>,
    },
}

impl LocalAssist {
    fn start(
        &mut self,
        ctx: &egui::Context,
        activity: &'static str,
        task: impl FnOnce() -> AssistReply + Send + 'static,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        self.message.clear();
        self.activity = activity;
        self.pending = Some(receiver);
        std::thread::spawn(move || {
            let _ = sender.send(task());
            ctx.request_repaint();
        });
    }

    fn poll(&mut self, checkpoint: &str, draft: &str, preview: &mut Option<AssistPreview>) {
        use std::sync::mpsc::TryRecvError;
        let Some(receiver) = &self.pending else {
            return;
        };
        let reply = match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            reply => reply,
        };
        self.pending = None;
        self.rewrite_progress = None;
        self.model_progress = None;
        #[cfg(windows)]
        {
            self.installer_progress = None;
        }
        match reply {
            Ok(AssistReply::Models(Ok(models)) | AssistReply::Download(Ok(models))) => {
                self.connected = true;
                self.models = models;
                if !self.models.contains(&self.selected) {
                    self.selected = self.models.first().cloned().unwrap_or_default();
                }
                if self.models.is_empty() {
                    self.message = "Ollama is running. No local models are installed.".into();
                }
            }
            Ok(AssistReply::Models(Err(error))) => {
                self.connected = false;
                self.models.clear();
                self.selected.clear();
                self.message = error;
            }
            Ok(AssistReply::Download(Err(error))) => self.message = error,
            #[cfg(windows)]
            Ok(AssistReply::Installer(result)) => match result {
                Ok(path) => {
                    self.installer = Some(path);
                    self.message =
                        "Ollama installer downloaded. Click Open Installer to run setup.".into();
                }
                Err(error) => self.message = error,
            },
            #[cfg(windows)]
            Ok(AssistReply::InstallerOpened(result)) => {
                self.message = match result {
                    Ok(()) => "After Ollama setup finishes, click Check Ollama.".into(),
                    Err(error) => error,
                };
            }
            Ok(AssistReply::Rewrite {
                checkpoint: source_model,
                source,
                result,
            }) => {
                if source_model != checkpoint || source != draft {
                    self.message = "The draft or image model changed. Run AI Assist again.".into();
                } else {
                    match result {
                        Ok(suggestion) => *preview = Some(AssistPreview { source, suggestion }),
                        Err(error) => self.message = error,
                    }
                }
            }
            Err(TryRecvError::Disconnected) => {
                self.message = "AI Assist stopped unexpectedly.".into()
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn show_status(&self, ui: &mut egui::Ui) {
        if let Some((started, progress)) = &self.rewrite_progress {
            let text = progress.lock().unwrap().clone();
            let stage = if text.is_empty() {
                "Waiting for model"
            } else {
                "Writing suggestion"
            };
            ui.add(
                egui::ProgressBar::new(0.0)
                    .animate(true)
                    .text(format!("{stage} · {:.0}s", started.elapsed().as_secs_f64())),
            );
            if !text.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("ai-assist-stream")
                    .max_height(100.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.label(text);
                    });
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        if let Some(progress) = &self.model_progress {
            let progress = progress.lock().unwrap().clone();
            let mib = 1024.0 * 1024.0;
            ui.label(self.activity);
            ui.label(&progress.status);
            let bar = if progress.total > 0 {
                egui::ProgressBar::new(progress.completed as f32 / progress.total as f32).text(
                    format!(
                        "{:.1} / {:.1} MiB",
                        progress.completed as f64 / mib,
                        progress.total as f64 / mib
                    ),
                )
            } else {
                egui::ProgressBar::new(0.0)
                    .animate(true)
                    .text("Preparing download…")
            };
            ui.add(bar);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }

        #[cfg(windows)]
        if let Some(progress) = &self.installer_progress {
            let (received, total) = *progress.lock().unwrap();
            let mib = 1024.0 * 1024.0;
            let received_mib = received as f64 / mib;
            let bar = if let Some(total) = total.filter(|total| *total > 0) {
                egui::ProgressBar::new((received as f64 / total as f64) as f32)
                    .text(format!("{received_mib:.1} / {:.1} MiB", total as f64 / mib))
            } else {
                egui::ProgressBar::new(0.0)
                    .animate(true)
                    .text(format!("{received_mib:.1} MiB downloaded"))
            };
            ui.label(self.activity);
            ui.add(bar);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        if self.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(self.activity);
            });
        } else if !self.message.is_empty() {
            ui.label(&self.message);
        }
    }
}

fn show_assist(
    ui: &mut egui::Ui,
    preview: &mut Option<AssistPreview>,
    local: &mut LocalAssist,
    draft: &mut String,
    checkpoint: &str,
    profile: Profile,
) {
    local.poll(checkpoint, draft, preview);
    let idle = local.pending.is_none();
    let has_draft = !draft.trim().is_empty();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(idle && has_draft, egui::Button::new("Quick Assist"))
            .on_hover_text("Preview rule-based cleanup for this draft.")
            .clicked()
        {
            *preview = Some(AssistPreview {
                source: draft.clone(),
                suggestion: quick_assist::suggest(draft, profile),
            });
        }
        if ui
            .add_enabled(
                idle,
                egui::Button::new(if local.open {
                    "Hide AI Assist"
                } else {
                    "AI Assist"
                }),
            )
            .clicked()
        {
            local.open = !local.open;
            if local.open && local.models.is_empty() {
                local.start(ui.ctx(), "Checking Ollama…", || {
                    AssistReply::Models(ai_assist::models())
                });
            }
        }
    });
    if local.open {
        ui.label("AI Assist uses an installed Ollama text model on this computer.");
        ui.small("Review the rewrite for changed details before applying it.");
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(idle && !local.models.is_empty(), |ui| {
                egui::ComboBox::from_id_salt("ai-assist-model")
                    .selected_text(if local.selected.is_empty() {
                        "No local model selected"
                    } else {
                        &local.selected
                    })
                    .show_ui(ui, |ui| {
                        for model in &local.models {
                            ui.selectable_value(&mut local.selected, model.clone(), model);
                        }
                    });
            });
            if ui
                .add_enabled(idle, egui::Button::new("Refresh Models"))
                .clicked()
            {
                local.start(ui.ctx(), "Checking Ollama…", || {
                    AssistReply::Models(ai_assist::models())
                });
            }
            if ui
                .add_enabled(
                    idle && has_draft && !local.selected.is_empty(),
                    egui::Button::new("Rewrite Draft"),
                )
                .clicked()
            {
                let source = draft.clone();
                let checkpoint = checkpoint.to_owned();
                let model = local.selected.clone();
                *preview = None;
                let progress = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                local.rewrite_progress = Some((std::time::Instant::now(), progress.clone()));
                local.start(ui.ctx(), "Rewriting with local Ollama…", move || {
                    let result = ai_assist::rewrite(&model, &source, profile, |draft| {
                        let mut text = progress.lock().unwrap();
                        text.clear();
                        text.push_str(draft);
                    });
                    AssistReply::Rewrite {
                        checkpoint,
                        source,
                        result,
                    }
                });
            }
        });
        local.show_status(ui);
        if local.models.is_empty() && local.pending.is_none() {
            ui.small(
                "Set up Ollama in Setup and Maintenance, then download a local text model in Model Library.",
            );
        }
    }
    let Some(assist) = preview else {
        return;
    };
    ui.label("Suggested Prompt");
    let stale = assist.source != *draft;
    if stale {
        ui.label("The scratchpad changed. Generate a new suggestion to apply it.");
    } else if assist.suggestion == *draft {
        ui.label("Quick Assist found no changes to suggest. You can edit the result below.");
    }
    let output = egui::TextEdit::multiline(&mut assist.suggestion)
        .desired_width(f32::INFINITY)
        .desired_rows(3)
        .show(ui);
    crate::interaction::text_menu(
        ui,
        &output.response,
        &assist.suggestion,
        &output.state,
        |_| {},
    );

    let close = ui
        .horizontal_wrapped(|ui| {
            let can_apply =
                !stale && !assist.suggestion.trim().is_empty() && assist.suggestion != *draft;
            let applied = ui
                .add_enabled(can_apply, egui::Button::new("Apply to Scratchpad"))
                .clicked();
            if applied {
                draft.clone_from(&assist.suggestion);
            }
            ui.button("Close Preview").clicked() || applied
        })
        .inner;
    if close {
        *preview = None;
    }
}

#[derive(Default)]
struct Scratchpad {
    open: bool,
    edit_as_text: bool,
    draft: String,
    name: String,
    model: Option<String>,
    drafts: std::collections::BTreeMap<String, (String, String)>,
    saved: SavedPrompts,
    message: String,
}

impl Scratchpad {
    fn select_model(&mut self, checkpoint: &str) {
        if self.model.as_deref() == Some(checkpoint) {
            return;
        }
        if let Some(previous) = self.model.replace(checkpoint.to_owned()) {
            self.drafts.insert(
                previous,
                (
                    std::mem::take(&mut self.draft),
                    std::mem::take(&mut self.name),
                ),
            );
        }
        (self.draft, self.name) = self.drafts.remove(checkpoint).unwrap_or_default();
        self.message.clear();
    }

    fn load() -> Self {
        let saved = profile_file("saved-prompts.json")
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            saved,
            ..Self::default()
        }
    }

    fn tag_target<'a>(&'a mut self, initial_prompt: &'a mut String) -> &'a mut String {
        if self.open {
            &mut self.draft
        } else {
            initial_prompt
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        initial_prompt: &mut String,
        user_tags: Option<&UserTags>,
        spelling: &mut PromptSpelling,
    ) {
        let label = if self.open {
            "Close Scratchpad"
        } else {
            "Open Scratchpad"
        };
        if ui.button(label).clicked() {
            self.open = !self.open;
        }
        if !self.open {
            return;
        }
        let text_id = PromptSpelling::editor_id(ui, SCRATCHPAD_FIELD);
        let slots = user_tags
            .map(|tags| tag_slots(&self.draft, &tags.saved))
            .unwrap_or_default();
        if self.edit_as_text || slots.is_empty() || ui.memory(|memory| memory.has_focus(text_id)) {
            spelling.edit(
                ui,
                &mut self.draft,
                SCRATCHPAD_FIELD,
                "Draft a prompt here…",
                SCRATCHPAD_ROWS,
            );
        } else {
            self.show_tag_editor(ui, &slots);
        }
        self.show_editor_actions(ui, initial_prompt, user_tags.is_some());
        let name_width = (ui.available_width() - 255.0).clamp(96.0, 220.0);
        ui.horizontal_wrapped(|ui| {
            let height = ui
                .spacing()
                .interact_size
                .y
                .max(ui.text_style_height(&egui::TextStyle::Body) + 4.0)
                .max(
                    ui.text_style_height(&egui::TextStyle::Button)
                        .max(ui.spacing().icon_width)
                        + 2.0 * ui.spacing().button_padding.y,
                );
            ui.spacing_mut().interact_size.y = height;
            let name = egui::TextEdit::singleline(&mut self.name)
                .hint_text("Prompt name")
                .desired_width(name_width)
                .min_size(egui::vec2(0.0, height))
                .show(ui);
            crate::interaction::text_menu(ui, &name.response, &self.name, &name.state, |_| {});
            if ui
                .add_enabled(
                    !self.name.trim().is_empty() && !self.draft.trim().is_empty(),
                    egui::Button::new("Save Prompt").min_size(egui::vec2(0.0, height)),
                )
                .clicked()
            {
                self.save_prompt();
            }
            ui.add_enabled_ui(!self.saved.is_empty(), |ui| {
                egui::ComboBox::from_id_salt("saved-prompts")
                    .selected_text("Load saved prompt")
                    .show_ui(ui, |ui| {
                        for (name, prompt) in &self.saved {
                            if ui.selectable_label(false, name).clicked() {
                                self.draft.clone_from(prompt);
                                self.name.clone_from(name);
                                self.message.clear();
                            }
                        }
                    });
            });
        });
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
    }

    fn save_prompt(&mut self) {
        let name = self.name.trim();
        if self.saved.contains_key(name) {
            self.message = "That name is already saved. Enter a different name.".into();
            return;
        }
        let mut saved = self.saved.clone();
        saved.insert(name.to_owned(), self.draft.clone());
        let result = serde_json::to_vec(&saved)
            .map_err(std::io::Error::from)
            .and_then(|bytes| write_profile_file("saved-prompts.json", &bytes));
        match result {
            Ok(()) => {
                self.saved = saved;
                self.message = "Prompt saved.".into();
            }
            Err(error) => self.message = format!("Could not save prompt. {error}"),
        }
    }

    fn show_editor_actions(
        &mut self,
        ui: &mut egui::Ui,
        initial_prompt: &mut String,
        danbooru: bool,
    ) {
        let edit_label = if self.edit_as_text {
            "Show draggable tags"
        } else {
            "Edit as text"
        };
        let transfer_label = "Transfer to Initial Prompt";
        let transfer = |ui: &mut egui::Ui, draft: &String| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled(!draft.trim().is_empty(), egui::Button::new(transfer_label))
                    .on_hover_text("Replace the initial prompt with this draft.")
                    .clicked()
            })
            .inner
        };
        let button_width = |label: &str| {
            ui.painter()
                .layout_no_wrap(
                    label.into(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    ui.visuals().text_color(),
                )
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x
                + ui.spacing().item_spacing.x
        };
        let mut row_width = button_width("Clear Scratchpad") + button_width(transfer_label);
        if danbooru {
            row_width += button_width(edit_label);
        }
        let fits = row_width <= ui.available_width();
        let transferred = ui
            .horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.draft.is_empty(),
                        egui::Button::new("Clear Scratchpad"),
                    )
                    .clicked()
                {
                    self.draft.clear();
                    self.message.clear();
                }
                if danbooru && ui.button(edit_label).clicked() {
                    self.edit_as_text = !self.edit_as_text;
                }
                fits && transfer(ui, &self.draft)
            })
            .inner;
        if transferred || (!fits && transfer(ui, &self.draft)) {
            initial_prompt.clone_from(&self.draft);
        }
    }

    fn show_tag_editor(&mut self, ui: &mut egui::Ui, slots: &[std::ops::Range<usize>]) {
        let mut target = None;
        let mut remove = None;
        let mut edits = Vec::new();
        let text_style = ui
            .style()
            .override_text_style
            .as_ref()
            .unwrap_or(&egui::TextStyle::Body);
        let field = &ui.visuals().widgets.inactive;
        let min_height =
            SCRATCHPAD_ROWS as f32 * ui.text_style_height(text_style) - 2.0 * field.bg_stroke.width;
        egui::Frame::new()
            .fill(ui.visuals().text_edit_bg_color())
            .corner_radius(field.corner_radius)
            .stroke(field.bg_stroke)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                let row_width = ui.available_width();
                ui.set_min_width(row_width);
                let mut chips = Vec::with_capacity(slots.len());
                ui.horizontal_wrapped(|ui| {
                    let mut start = 0;
                    for (index, slot) in slots.iter().enumerate() {
                        edit_fragment(ui, &self.draft, start..slot.start, index, &mut edits);
                        let tag = &self.draft[slot.clone()];
                        let text_width = ui
                            .painter()
                            .layout_no_wrap(
                                tag.to_owned(),
                                egui::TextStyle::Button.resolve(ui.style()),
                                ui.visuals().text_color(),
                            )
                            .size()
                            .x;
                        let chip_width = (text_width + 2.0 * ui.spacing().button_padding.x)
                            .max(ui.spacing().interact_size.x)
                            .min(row_width);
                        let remaining = ui.available_size_before_wrap().x;
                        if remaining + 0.5 < chip_width && remaining + 0.5 < row_width {
                            ui.end_row();
                        }
                        let chip = ui
                            .push_id(("scratchpad-tag", index), |ui| {
                                ui.add(
                                    egui::Button::new(tag)
                                        .sense(egui::Sense::click_and_drag())
                                        .truncate(),
                                )
                                .on_hover_cursor(egui::CursorIcon::Grab)
                            })
                            .inner;
                        chip.dnd_set_drag_payload(TagDrag(index));
                        chips.push(chip.rect);
                        chip.context_menu(|ui| {
                            if ui.button("Remove").clicked() {
                                remove = Some(slot.clone());
                                ui.close();
                            }
                        });
                        start = slot.end;
                    }
                    edit_fragment(
                        ui,
                        &self.draft,
                        start..self.draft.len(),
                        slots.len(),
                        &mut edits,
                    );
                });
                let extra_height = chips
                    .first()
                    .and_then(|first| chips.iter().find(|chip| chip.top() >= first.bottom()))
                    .zip(chips.last())
                    .map_or(0.0, |(second_row, last)| {
                        if last.top() >= second_row.bottom() {
                            last.top() - second_row.top()
                        } else {
                            0.0
                        }
                    });
                ui.expand_to_include_y(ui.min_rect().top() + min_height + extra_height);
                if let Some(source) = egui::DragAndDrop::payload::<TagDrag>(ui.ctx())
                    && source.0 < chips.len()
                    && ui.rect_contains_pointer(ui.min_rect())
                    && let Some(pointer) = ui.ctx().pointer_interact_pos()
                {
                    target = chip_drop_gap(&chips, pointer);
                    if let Some(gap) = target {
                        paint_tag_preview(ui, &self.draft, slots, &chips, source.0, gap);
                    }
                }
                if target.is_none() {
                    for (index, chip) in chips.iter().enumerate() {
                        animate_chip_center(ui, index, chip.center(), 0.0);
                    }
                }
            });
        if let Some(slot) = remove {
            remove_tag(&mut self.draft, slot);
        } else if let Some(gap) = target
            && ui.input(|input| input.pointer.primary_released())
            && let Some(source) = egui::DragAndDrop::take_payload::<TagDrag>(ui.ctx())
        {
            reorder_tags(&mut self.draft, slots, source.0, gap);
        } else {
            for (range, text) in edits.into_iter().rev() {
                self.draft.replace_range(range, &text);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct TagDrag(usize);

fn chip_drop_gap(chips: &[egui::Rect], pointer: egui::Pos2) -> Option<usize> {
    let row = chips
        .iter()
        .enumerate()
        .filter(|(_, rect)| rect.y_range().contains(pointer.y));
    let mut gap = None;
    for (index, rect) in row {
        if pointer.x < rect.center().x {
            return Some(index);
        }
        gap = Some(index + 1);
    }
    gap
}

fn paint_tag_preview(
    ui: &egui::Ui,
    draft: &str,
    slots: &[std::ops::Range<usize>],
    chips: &[egui::Rect],
    source: usize,
    gap: usize,
) {
    // Move the labels between the existing chip positions without changing the
    // layout that the pointer is targeting. The final drop may wrap new rows.
    let mut order: Vec<_> = (0..chips.len()).collect();
    order.remove(source);
    let landing = if gap > source { gap - 1 } else { gap };
    order.insert(landing, source);
    let painter = ui.painter();
    let visuals = ui.visuals();
    for &rect in chips {
        painter.rect_filled(rect, 0.0, visuals.text_edit_bg_color());
    }
    for (&rect, index) in chips.iter().zip(order) {
        let center = animate_chip_center(ui, index, rect.center(), 0.12);
        let rect = egui::Rect::from_center_size(center, rect.size());
        let style = &visuals.widgets.inactive;
        let fill = if index == source {
            visuals.selection.bg_fill
        } else {
            style.weak_bg_fill
        };
        painter.rect(
            rect,
            style.corner_radius,
            fill,
            style.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let text = &draft[slots[index].clone()];
        let mut font = egui::TextStyle::Button.resolve(ui.style());
        let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), visuals.text_color());
        let width = (rect.width() - 4.0).max(1.0);
        font.size *= (width / galley.size().x.max(1.0)).min(1.0);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            font,
            visuals.text_color(),
        );
    }
}

fn animate_chip_center(
    ui: &egui::Ui,
    index: usize,
    center: egui::Pos2,
    seconds: f32,
) -> egui::Pos2 {
    let id = ui.make_persistent_id(("scratchpad-chip-animation", index));
    egui::pos2(
        ui.ctx()
            .animate_value_with_time(id.with("x"), center.x, seconds),
        ui.ctx()
            .animate_value_with_time(id.with("y"), center.y, seconds),
    )
}

fn remove_tag(draft: &mut String, slot: std::ops::Range<usize>) {
    let after = draft[slot.end..].trim_start();
    let before = draft[..slot.start].trim_end();
    let range = if let Some(rest) = after.strip_prefix(',') {
        slot.start..draft.len() - rest.trim_start().len()
    } else if let Some(rest) = before.strip_suffix(',') {
        rest.trim_end().len()..slot.end
    } else {
        slot
    };
    draft.replace_range(range, "");
    if draft.trim().is_empty() {
        draft.clear();
    }
}

fn edit_fragment(
    ui: &mut egui::Ui,
    draft: &str,
    range: std::ops::Range<usize>,
    index: usize,
    edits: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    let fragment = &draft[range.clone()];
    if fragment
        .trim_matches(|ch: char| ch.is_whitespace() || ch == ',')
        .is_empty()
    {
        return;
    }
    let mut text = fragment.to_owned();
    ui.push_id(("scratchpad-prose", index), |ui| {
        let output = egui::TextEdit::multiline(&mut text)
            .desired_rows(1)
            .desired_width(ui.available_width())
            .show(ui);
        crate::interaction::text_menu(ui, &output.response, &text, &output.state, |_| {});
        if output.response.changed() {
            edits.push((range, text));
        }
    });
}

fn tag_slots(draft: &str, saved: &TagCategories) -> Vec<std::ops::Range<usize>> {
    let known = COMMON_TAGS
        .iter()
        .flat_map(|(_, tags)| tags.iter().copied())
        .chain(saved.values().flatten().map(String::as_str));
    let mut slots = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut escaped = false;
    for (end, ch) in draft
        .char_indices()
        .chain(std::iter::once((draft.len(), ',')))
    {
        if escaped && end < draft.len() {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let fragment = &draft[start..end];
                let tag = fragment.trim();
                if known.clone().any(|candidate| same_tag(candidate, tag)) {
                    let first = start + fragment.len() - fragment.trim_start().len();
                    slots.push(first..first + tag.len());
                }
                start = end + 1;
            }
            _ => {}
        }
    }
    slots
}

fn reorder_tags(draft: &mut String, slots: &[std::ops::Range<usize>], source: usize, gap: usize) {
    if source >= slots.len() || gap > slots.len() || gap == source || gap == source + 1 {
        return;
    }
    let mut tags: Vec<_> = slots
        .iter()
        .map(|slot| draft[slot.clone()].to_owned())
        .collect();
    let moved = tags.remove(source);
    let target = if gap > source { gap - 1 } else { gap };
    tags.insert(target, moved);
    for (slot, tag) in slots.iter().zip(tags).rev() {
        draft.replace_range(slot.clone(), &tag);
    }
}

fn show_guide(
    ui: &mut egui::Ui,
    checkpoint: &str,
    tag_target: &mut String,
    user_tags: &mut UserTags,
) {
    if checkpoint.is_empty() {
        ui.label("Select a generation model to see its prompt guide.");
        return;
    }
    let (guide, source) = guide_for(checkpoint);
    ui.heading(checkpoint);
    ui.label(guide.structure);
    ui.separator();
    ui.strong("Example prompt");
    ui.label(guide.example);
    if ui.button("Copy example").clicked() {
        ui.ctx().copy_text(guide.example.to_owned());
    }
    ui.separator();
    ui.strong("Negative prompts");
    ui.label(if ai_generation::ignores_negative(checkpoint) {
                "PixelForge does not use negative prompts with this model. Describe the result you want in the main prompt."
            } else {
                guide.negative
            });
    if let Some(source) = source {
        ui.hyperlink_to("Model documentation", source);
    }
    if guide.danbooru {
        ui.separator();
        ui.label("Click a tag to add it to the open scratchpad or initial prompt.");
        user_tags.show(ui, tag_target);
        ui.hyperlink_to(
            "Danbooru Prompt Translator (opens in browser)",
            "https://anythingtranslate.com/translators/danbooru-prompt-translator/",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_assist_previews_only_results_for_the_current_draft_and_checkpoint() {
        for (checkpoint, draft, expected) in [
            ("FLUX", "two cyclists, back view", true),
            ("Pony", "two cyclists, back view", false),
            ("FLUX", "two cyclists, front view", false),
        ] {
            let (sender, receiver) = std::sync::mpsc::channel();
            let mut assist = LocalAssist {
                pending: Some(receiver),
                ..Default::default()
            };
            sender
                .send(AssistReply::Rewrite {
                    checkpoint: "FLUX".into(),
                    source: "two cyclists, back view".into(),
                    result: Ok("Rear view of two cyclists.".into()),
                })
                .unwrap();
            let mut preview = None;
            assist.poll(checkpoint, draft, &mut preview);
            assert!(assist.pending.is_none());
            assert_eq!(
                preview.as_ref().map(|preview| preview.suggestion.as_str()),
                expected.then_some("Rear view of two cyclists.")
            );
        }
    }

    #[test]
    fn quick_assist_uses_the_selected_checkpoint_guide() {
        let input = "score_9,back view";
        let pony = guide_for("models/CyberRealisticPony_V18.0_F16.safetensors").0;
        assert_eq!(
            quick_assist::suggest(input, pony.assist_profile),
            "score_9, back view"
        );

        for checkpoint in [ai_generation::Z_IMAGE, ai_generation::FLUX_KLEIN] {
            let guide = guide_for(checkpoint).0;
            assert_eq!(
                quick_assist::suggest("back view", guide.assist_profile),
                "back view."
            );
        }

        let custom = guide_for("custom-pony.safetensors").0;
        assert_eq!(quick_assist::suggest(input, custom.assist_profile), input);
    }

    #[test]
    fn clicking_scratchpad_in_prompt_guide_draws_caret_and_accepts_text() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut guide = PromptGuide {
            open: true,
            scratchpad: Scratchpad {
                open: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut spelling = PromptSpelling::default();
        let mut initial = String::new();
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| guide.show(ctx, "Test model", &mut initial, &mut spelling),
            )
        };
        frame(vec![]);
        let output = frame(vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Draft a prompt here…" => {
                    Some(text.pos + egui::vec2(4.0, 6.0))
                }
                _ => None,
            })
            .expect("scratchpad hint");
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        let output = frame(vec![egui::Event::Text("red coat".into())]);
        assert_eq!(guide.scratchpad.draft, "red coat");
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::LineSegment { points, stroke }
                if points[0].x == points[1].x && stroke.color.a() > 0 && stroke.width == 2.0
                    && shape.clip_rect.contains(points[0].lerp(points[1], 0.5))
        )));
    }

    #[test]
    fn saved_prompt_controls_have_matching_visible_heights() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        ctx.style_mut(|style| {
            style.spacing.icon_width = 22.0;
            style.spacing.interact_size.y = 26.0;
            for text_style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                style.text_styles.get_mut(&text_style).unwrap().size = 16.0;
            }
        });
        let mut scratchpad = Scratchpad {
            open: true,
            draft: "solo".into(),
            name: "Test Prompt".into(),
            saved: SavedPrompts::from([("Example".into(), "jacket".into())]),
            ..Default::default()
        };
        let mut initial = String::new();
        let mut spelling = PromptSpelling::default();
        spelling.enabled = false;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                scratchpad.show(ui, &mut initial, None, &mut spelling)
            });
        });
        let control_rect = |label: &str| {
            let center = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .unwrap();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect.contains(center) => Some(rect.rect),
                    _ => None,
                })
                .min_by(|left, right| left.area().total_cmp(&right.area()))
                .unwrap()
        };
        let name = control_rect("Test Prompt");
        for label in ["Save Prompt", "Load saved prompt"] {
            let button = control_rect(label);
            assert_eq!(button.y_range(), name.y_range(), "{label}");
        }
    }

    #[test]
    fn chips_wrap_as_units_and_editor_grows_after_two_rows() {
        let ctx = egui::Context::default();
        let render = |draft: &str| {
            let mut scratchpad = Scratchpad {
                draft: draft.into(),
                ..Default::default()
            };
            let mut height = 0.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(135.0, 500.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let slots = tag_slots(&scratchpad.draft, &TagCategories::new());
                    height = ui
                        .vertical(|ui| scratchpad.show_tag_editor(ui, &slots))
                        .response
                        .rect
                        .height();
                });
            });
            (output, height)
        };
        let (_, single) = render("solo");
        let (_, same_row) = render("solo, jacket");
        let (wrapped, second_row) = render("solo, jacket, looking_at_viewer");
        let (_, third_row) = render("solo, jacket, looking_at_viewer, standing, full_body");
        let top = |label: &str| {
            wrapped
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => Some(text.pos.y),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(single, same_row);
        assert!(top("looking_at_viewer") > top("jacket"));
        assert_eq!(second_row, single);
        assert!(third_row > single);
    }

    #[test]
    fn switching_models_restores_each_draft() {
        let mut scratchpad = Scratchpad::default();
        scratchpad.select_model("Pony");
        scratchpad.draft = "solo, jacket".into();
        scratchpad.name = "Character".into();
        scratchpad.select_model("SDXL");
        assert!(scratchpad.draft.is_empty());
        assert!(scratchpad.name.is_empty());
        scratchpad.draft = "A landscape".into();
        scratchpad.select_model("Pony");
        assert_eq!(scratchpad.draft, "solo, jacket");
        assert_eq!(scratchpad.name, "Character");
        scratchpad.select_model("SDXL");
        assert_eq!(scratchpad.draft, "A landscape");
    }

    #[test]
    fn removing_chips_preserves_other_prompt_text_and_separators() {
        let mut draft = "solo, a rainy city, jacket, outdoors".to_owned();
        let saved = TagCategories::new();
        let slots = tag_slots(&draft, &saved);
        remove_tag(&mut draft, slots[1].clone());
        assert_eq!(draft, "solo, a rainy city, outdoors");
        let slots = tag_slots(&draft, &saved);
        remove_tag(&mut draft, slots[1].clone());
        remove_tag(&mut draft, slots[0].clone());
        assert_eq!(draft, "a rainy city");
    }

    #[test]
    fn dragging_a_chip_to_the_end_keeps_the_editor_height_stable() {
        let ctx = egui::Context::default();
        let mut scratchpad = Scratchpad {
            draft: "solo, jacket, outdoors".into(),
            ..Default::default()
        };
        let mut frame = |events| {
            let mut height = 0.0;
            let output = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let slots = tag_slots(&scratchpad.draft, &TagCategories::new());
                        height = ui
                            .vertical(|ui| scratchpad.show_tag_editor(ui, &slots))
                            .response
                            .rect
                            .height();
                    });
                },
            );
            (output, height)
        };
        let text_center = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .unwrap()
        };
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        let (idle, original_height) = frame(vec![]);
        let chip = text_center(&idle, "solo");
        let end = text_center(&idle, "outdoors") + egui::vec2(45.0, 0.0);
        frame(vec![egui::Event::PointerMoved(chip), pointer(chip, true)]);
        let (_, drag_height) = frame(vec![egui::Event::PointerMoved(
            chip + egui::vec2(14.0, 0.0),
        )]);
        assert_eq!(drag_height, original_height);
        let (_, hovered_height) = frame(vec![egui::Event::PointerMoved(end)]);
        assert_eq!(hovered_height, original_height);
        frame(vec![pointer(end, false)]);
        assert_eq!(scratchpad.draft, "jacket, outdoors, solo");
    }

    #[test]
    fn reordering_tags_preserves_prose_and_nested_prompt_syntax() {
        let saved = TagCategories::from([("Clothing".into(), vec!["raincoat".into()])]);
        let mut draft =
            "solo, a rainy city, (dramatic light, smile:1.2), jacket, raincoat".to_owned();
        let slots = tag_slots(&draft, &saved);
        assert_eq!(
            slots
                .iter()
                .map(|slot| &draft[slot.clone()])
                .collect::<Vec<_>>(),
            ["solo", "jacket", "raincoat"]
        );
        reorder_tags(&mut draft, &slots, 2, 0);
        assert_eq!(
            draft,
            "raincoat, a rainy city, (dramatic light, smile:1.2), solo, jacket"
        );
        let slots = tag_slots(&draft, &saved);
        reorder_tags(&mut draft, &slots, 0, slots.len());
        assert_eq!(
            draft,
            "solo, a rainy city, (dramatic light, smile:1.2), jacket, raincoat"
        );
    }

    #[test]
    fn tag_destination_follows_scratchpad_visibility() {
        let mut scratchpad = Scratchpad::default();
        let mut initial = "A portrait".to_owned();
        insert_tag(scratchpad.tag_target(&mut initial), "solo");
        scratchpad.open = true;
        insert_tag(scratchpad.tag_target(&mut initial), "upper_body");
        scratchpad.open = false;
        insert_tag(scratchpad.tag_target(&mut initial), "outdoors");
        assert_eq!(initial, "A portrait, solo, outdoors");
        assert_eq!(scratchpad.draft, "upper_body");
    }

    #[test]
    fn guide_layout_shares_top_left_and_preserves_model_sizes() {
        let mut guide = PromptGuide {
            open: true,
            anchor: Some([80.0, 95.0]),
            sizes: GuideSizes::from([
                ("Model A".into(), [350.0, 300.0]),
                ("Model B".into(), [480.0, 450.0]),
            ]),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut prompt = String::new();
        let mut spelling = PromptSpelling::default();
        for checkpoint in ["Model A", "Model B", "Model A"] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                guide.show(ctx, checkpoint, &mut prompt, &mut spelling)
            });
            assert_eq!(guide.anchor, Some([80.0, 95.0]));
        }
        #[derive(Default)]
        struct LayoutStorage(Option<String>);
        impl eframe::Storage for LayoutStorage {
            fn get_string(&self, key: &str) -> Option<String> {
                assert_eq!(key, LAYOUT_KEY);
                self.0.clone()
            }
            fn set_string(&mut self, key: &str, value: String) {
                assert_eq!(key, LAYOUT_KEY);
                self.0 = Some(value);
            }
            fn flush(&mut self) {}
        }
        let mut storage = LayoutStorage::default();
        guide.save_layout(&mut storage);
        let mut restored = PromptGuide::default();
        restored.restore_layout(&storage);
        assert_eq!(restored.anchor, guide.anchor);
        assert_eq!(restored.sizes["Model A"], [350.0, 300.0]);
        assert_eq!(restored.sizes["Model B"], [480.0, 450.0]);
    }

    #[test]
    fn curated_generation_models_have_guides() {
        for model in MODELS.iter().filter(|model| model.generation) {
            assert_eq!(guide_for(model.name).1, Some(model.page), "{}", model.name);
            for file in model
                .files
                .iter()
                .filter(|file| file.directory == "checkpoints")
            {
                let checkpoint = format!("local\\{}", file.file.to_ascii_uppercase());
                assert_eq!(guide_for(&checkpoint).1, Some(model.page));
            }
        }
        let (pony, _) = guide_for("CyberRealisticPony_V18.0_F16.safetensors");
        assert!(pony.example.starts_with("score_9, score_8_up, score_7_up"));
        assert!(guide_for("custom-pony.safetensors").1.is_none());
    }

    #[test]
    fn inserts_tags_in_click_order_and_preserves_existing_prompt() {
        let mut prompt = "A portrait in a woodshop, looking at viewer".to_owned();
        for tag in [
            "score_7_up",
            "score_9",
            "score_8_up",
            "looking_at_viewer",
            "upper_body",
        ] {
            insert_tag(&mut prompt, tag);
        }
        assert_eq!(
            prompt,
            "A portrait in a woodshop, looking at viewer, score_7_up, score_9, score_8_up, upper_body"
        );
        let mut prompt = "score_9".to_owned();
        insert_tag(&mut prompt, "score_8_up");
        assert_eq!(prompt, "score_9, score_8_up");
        insert_tag(&mut prompt, "highres");
        assert_eq!(prompt, "score_9, score_8_up, highres");
    }

    #[test]
    fn saved_tags_keep_categories_and_legacy_tags_move_to_other() {
        let legacy = decode_tags(br#"["raincoat"]"#).unwrap();
        assert_eq!(legacy["Other"], ["raincoat"]);
        let categorized =
            decode_tags(br#"{"Quality":["highres"],"Subject":["raincoat"]}"#).unwrap();
        assert_eq!(categorized["Quality"], ["highres"]);
        assert_eq!(categorized["Subject"], ["raincoat"]);
    }
}
