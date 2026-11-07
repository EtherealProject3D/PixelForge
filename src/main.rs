#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod adjustments;
mod theme;
mod interaction;
mod numeric;
mod profile;
mod display;
mod startup;
mod compatibility;
mod resolution;
mod backend;
mod updater;
mod ai_assist;
mod ai_denoise;
mod ai_generation;
mod prompt_guide;
mod quick_assist;
mod ai_restoration;
mod ai_upscale;
mod analysis;
mod auto_enhance;
mod histogram;
#[allow(dead_code)]
mod masking;
mod model_installer;
mod dependencies;
mod project;
mod generation_project;
mod studio;
mod crop;
mod spelling;

use eframe::egui;
use egui::{Color32, ColorImage, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;

fn restoration_preset_settings(preset: usize) -> (u32, f32) {
    match preset {
        1 => (2, 0.80),
        2 => (0, 0.75),
        _ => (1, 0.65),
    }
}

/// Decode high-resolution PNG files for native AI inspection.
fn load_inspection_png(path: &std::path::Path, label: &str) -> Result<image::RgbaImage, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open {label}: {e}"))?;

    let reader = std::io::BufReader::new(file);

    let mut decoder = image::codecs::png::PngDecoder::new(reader)
        .map_err(|e| format!("Cannot initialize {label} decoder: {e}"))?;

    let mut limits = image::Limits::default();
    limits.max_alloc = Some(2 * 1024 * 1024 * 1024);

    image::ImageDecoder::set_limits(&mut decoder, limits)
        .map_err(|e| format!("{label} decoder limit: {e}"))?;

    image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Cannot decode {label}: {e}"))
        .map(|img| img.to_rgba8())
}

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
    ui_scale: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            pan: PanButton::Middle,
            fit: FitButton::Left,
            zoom_sensitivity: 1.0,
            ui_scale: 1.0,
        }
    }
}

impl Controls {
    fn config_path() -> Option<PathBuf> {
        Some(profile::settings_root()?.join("controls.cfg"))
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
                "ui_scale" => {
                    if let Ok(value) = value.trim().parse::<f32>() {
                        if value.is_finite() {
                            settings.ui_scale = value.clamp(0.75, 2.0);
                        }
                    }
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
            "pan={pan}\nfit={fit}\nzoom_sensitivity={}\nui_scale={}\n",
            self.zoom_sensitivity, self.ui_scale
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingOpen {
    Image,
    Project,
    Close,
}

#[derive(Clone)]
struct GenerationSettings {
    prompt: String,
    negative: String,
    auto_negative: bool,
    checkpoint: String,
    style: usize,
    ratio: usize,
    custom_size: bool,
    unlock_dimensions: bool,
    resolution: usize,
    dimension_step: u32,
    width: u32,
    height: u32,
    steps: u32,
    cfg: f32,
    seed: u64,
    refine_parent: Option<usize>,
    refine_strength: Option<f32>,
    refine_prompt: Option<String>,
    refine_masked: bool,
}

fn editing_checkpoint<'a>(generation: &'a str, editing: &'a str, is_edit: bool) -> &'a str {
    if is_edit && !editing.is_empty() { editing } else { generation }
}

fn conversation_context(settings: &GenerationSettings) -> String {
    match settings.refine_prompt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(edit) if !settings.prompt.trim().is_empty() => format!("{}, {}", settings.prompt.trim(), edit),
        Some(edit) => edit.to_owned(),
        None => settings.prompt.clone(),
    }
}

fn settings_json(s:&GenerationSettings)->serde_json::Value {serde_json::json!({"prompt":s.prompt,"negative":s.negative,"auto_negative":s.auto_negative,"checkpoint":s.checkpoint,"style":s.style,"ratio":s.ratio,"custom_size":s.custom_size,"unlock_dimensions":s.unlock_dimensions,"resolution":s.resolution,"dimension_step":s.dimension_step,"width":s.width,"height":s.height,"steps":s.steps,"cfg":s.cfg,"seed":s.seed,"refine_parent":s.refine_parent,"refine_strength":s.refine_strength,"refine_prompt":s.refine_prompt,"refine_masked":s.refine_masked})}
fn settings_from_json(v:&serde_json::Value)->GenerationSettings {GenerationSettings {prompt:v["prompt"].as_str().unwrap_or("").into(),negative:v["negative"].as_str().unwrap_or("").into(),auto_negative:v["auto_negative"].as_bool().unwrap_or(false),checkpoint:v["checkpoint"].as_str().unwrap_or("").into(),style:v["style"].as_u64().unwrap_or(0) as usize,ratio:v["ratio"].as_u64().unwrap_or(0) as usize,custom_size:v["custom_size"].as_bool().unwrap_or(false),unlock_dimensions:v["unlock_dimensions"].as_bool().unwrap_or(false),resolution:v["resolution"].as_u64().unwrap_or(0) as usize,dimension_step:resolution::valid_step(v["dimension_step"].as_u64().unwrap_or(64) as u32),width:v["width"].as_u64().unwrap_or(1024) as u32,height:v["height"].as_u64().unwrap_or(1024) as u32,steps:v["steps"].as_u64().unwrap_or(30) as u32,cfg:v["cfg"].as_f64().unwrap_or(7.0) as f32,seed:v["seed"].as_u64().unwrap_or(0),refine_parent:v["refine_parent"].as_u64().map(|n|n as usize),refine_strength:v["refine_strength"].as_f64().map(|n|n as f32),refine_prompt:v["refine_prompt"].as_str().map(str::to_owned),refine_masked:v["refine_masked"].as_bool().unwrap_or(false)}}

#[derive(Clone)]
struct GeneratedItem {
    group: Option<usize>,
    editor: studio::EditorState,
    settings: GenerationSettings,
    pixels: image::RgbaImage,
    texture: TextureHandle,
    seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ThumbnailGroup {
    name: String,
    collapsed: bool,
}

enum GroupDialog {
    Name { group: Option<usize>, name: String },
    Delete(usize),
}

#[derive(Clone, Copy)]
struct ThumbnailDrag {
    project: usize,
    index: usize,
}

#[derive(Clone, Copy)]
struct ThumbnailDrop {
    index: usize,
    group: Option<usize>,
    before: usize,
}

impl GeneratedItem {
    fn version_label(&self, index: usize) -> String {
        if self.editor.name.trim().is_empty() {format!("Version {}", index+1)}
        else {format!("{} · V{}", self.editor.name, index+1)}
    }
}

const GENERATION_STYLES: &[(&str, &str)] = &[
    ("Default", ""),
    (
        "Photorealistic",
        "unedited RAW photograph captured on a full-frame DSLR, authentic documentary photography, physically accurate natural lighting, realistic dynamic range, natural color science, subtle film grain, organic fine detail, true-to-life material textures, no CGI, no illustration, no artificial HDR, no oversharpening",
    ),
    (
        "Portrait Photography",
        "professional portrait photography, flattering studio light, natural skin texture, shallow depth of field",
    ),
    (
        "Landscape Photography",
        "real outdoor landscape photograph, full-frame camera, realistic geography and atmospheric perspective, naturally irregular trees and foliage, authentic terrain, subtle natural daylight, realistic haze, neutral color grading, restrained contrast, no fantasy scenery, no CGI, no artificial HDR, no oversaturation",
    ),
    (
        "Product",
        "commercial product photography, clean composition, studio lighting",
    ),
    (
        "Macro",
        "macro photography, intricate close-up detail, shallow depth of field",
    ),
    (
        "Cinematic",
        "cinematic film still, dramatic lighting, atmospheric color grading",
    ),
    (
        "3D Cartoon",
        "stylized 3D animated film, expressive shapes, polished materials",
    ),
    ("Anime", "anime illustration, clean linework, cel shading"),
    (
        "Watercolor",
        "traditional watercolor painting, textured paper, soft pigment washes",
    ),
    (
        "Oil Painting",
        "traditional oil painting, visible brush strokes, painterly texture",
    ),
    (
        "Pencil Sketch",
        "graphite pencil drawing, fine hatching, monochrome",
    ),
    (
        "Comic Book",
        "graphic novel illustration, bold ink lines, dynamic colors",
    ),
    (
        "Pixel Art",
        "pixel art, crisp pixel edges, limited color palette",
    ),
    (
        "Fantasy",
        "fantasy concept art, magical atmosphere, detailed worldbuilding",
    ),
    (
        "Sci-Fi",
        "science fiction concept art, futuristic atmosphere",
    ),
    (
        "Cyberpunk",
        "cyberpunk neon city aesthetic, dramatic futuristic lighting",
    ),
    (
        "Vintage Film",
        "analog film photograph, subtle grain, vintage color palette",
    ),
    (
        "Iconography",
        "professional iconography, simple recognizable symbols, clean geometric shapes, balanced proportions, strong silhouettes, precise edges, limited color palette, centered isolated design, minimal visual clutter",
    ),
    (
        "Logo Design",
        "professional brand identity concept, distinctive logo mark, balanced composition, memorable abstract symbol, elegant geometric construction, clean negative space, scalable visual simplicity",
    ),
    (
        "Vector Illustration",
        "clean flat vector-style illustration, smooth precise shapes, solid colors, crisp edges, balanced composition, modern editorial graphic design",
    ),
    (
        "Silhouette / Stencil",
        "bold high-contrast silhouette artwork, clean black and white shapes, solid filled regions, strong recognizable outlines, stencil-friendly graphic composition",
    ),
    (
        "Ink & Engraving",
        "traditional ink engraving, fine crosshatching, detailed etched linework, vintage printmaking aesthetics, precise black ink illustration",
    ),
    (
        "Low Poly",
        "stylized low-poly artwork, angular polygonal shapes, faceted surfaces, simplified geometry, carefully balanced colors",
    ),
    (
        "Isometric Art",
        "isometric illustration, consistent three-quarter projection, miniature scene composition, clean geometric structures, detailed stylized environment",
    ),
    (
        "Papercut Art",
        "layered paper cut illustration, stacked colored paper shapes, crisp cut edges, subtle dimensional shadows, handcrafted composition",
    ),
    (
        "Character Design",
        "professional character concept art, distinctive silhouette, coherent costume design, expressive pose, detailed original character illustration",
    ),
    (
        "Industrial Design",
        "professional industrial design concept visualization, functional product geometry, realistic materials, precise construction, clean presentation, innovative manufactured object",
    ),
    (
        "Synthwave",
        "retro futuristic synthwave artwork, 1980s inspired neon aesthetic, glowing magenta and cyan lighting, atmospheric retro technology, vibrant cinematic composition",
    ),
    (
        "Surrealism",
        "surrealist fine art, dreamlike atmosphere, imaginative impossible environments, unexpected visual combinations, coherent artistic composition",
    ),
];

fn automatic_negative_prompt(style: &str) -> &'static str {
    match style {
        "Iconography" | "Logo Design" => {
            "photorealistic, complex scenery, busy background, excessive detail, blurry edges, distorted shapes, illegible text, watermark, visual clutter"
        }
        "Vector Illustration" => {
            "photorealistic, photographic textures, muddy colors, blurry edges, excessive gradients, watermark, text"
        }
        "Silhouette / Stencil" => {
            "gradients, photographic textures, soft shadows, complex background, fine disconnected details, blurry edges, watermark, text"
        }
        "Ink & Engraving" => {
            "photorealistic, smooth digital shading, blurry linework, flat plastic textures, watermark, text"
        }
        "Low Poly" => {
            "photorealistic, smooth organic surfaces, excessive fine detail, blurry geometry, watermark, text"
        }
        "Isometric Art" => {
            "fisheye perspective, inconsistent projection, distorted geometry, blurry details, watermark, text"
        }
        "Papercut Art" => {
            "photorealistic, glossy plastic, blurry edges, flat lighting, watermark, text"
        }
        "Character Design" => {
            "inconsistent anatomy, extra limbs, distorted hands, duplicate features, blurry details, watermark, text"
        }
        "Industrial Design" => {
            "impossible geometry, distorted perspective, inconsistent materials, blurry details, watermark, text"
        }
        "Synthwave" => {
            "muted colors, flat lighting, natural daylight, blurry details, watermark, text"
        }
        "Surrealism" => {
            "mundane composition, repetitive scenery, low detail, blurry, watermark, text"
        }
        "Photorealistic"
        | "Portrait Photography"
        | "Landscape Photography"
        | "Product"
        | "Macro"
        | "Vintage Film" => {
            "cartoon, anime, illustration, CGI, plastic textures, artificial skin, distorted anatomy, unnatural colors, watermark, text"
        }
        "Cinematic" => {
            "cartoon, anime, flat lighting, artificial textures, washed out colors, watermark, text"
        }
        "3D Cartoon" => {
            "photorealistic, flat shading, muddy textures, distorted geometry, watermark, text"
        }
        "Anime" => {
            "photorealistic, 3D render, inconsistent linework, blurry, distorted anatomy, watermark, text"
        }
        "Watercolor" => {
            "photorealistic, 3D render, hard digital edges, plastic textures, watermark, text"
        }
        "Oil Painting" => {
            "photorealistic, 3D render, flat colors, digital artifacts, watermark, text"
        }
        "Pencil Sketch" => {
            "photorealistic, full color, painted textures, blurry lines, watermark, text"
        }
        "Comic Book" => "photorealistic, blurry linework, muddy colors, watermark, text",
        "Pixel Art" => {
            "photorealistic, smooth gradients, anti-aliasing, blurry pixels, watermark, text"
        }
        "Fantasy" => "modern photography, mundane setting, flat lighting, watermark, text",
        "Sci-Fi" => "historical setting, medieval architecture, low detail, watermark, text",
        "Cyberpunk" => "rural scenery, daylight photography, muted lighting, watermark, text",
        _ => "blurry, low quality, distorted, watermark, unwanted text",
    }
}
const GENERATION_SIZES: &[(&str, u32, u32)] = &[
    ("Square 1:1", 1, 1),
    ("Landscape 4:3", 4, 3),
    ("Portrait 3:4", 3, 4),
    ("Landscape 3:2", 3, 2),
    ("Portrait 2:3", 2, 3),
    ("Landscape 16:9", 16, 9),
    ("Portrait 9:16", 9, 16),
    ("Landscape 16:10", 16, 10),
    ("Portrait 10:16", 10, 16),
    ("Ultrawide 21:9", 21, 9),
    ("Portrait 9:21", 9, 21),
    ("Super Ultrawide 32:9", 32, 9),
    ("Portrait 9:32", 9, 32),
];

fn generation_dimensions(ratio: usize, resolution: usize, step: u32) -> (u32, u32) {
    let (_, rw, rh) = GENERATION_SIZES[ratio.min(GENERATION_SIZES.len() - 1)];
    resolution::preset_dimensions(rw, rh, resolution, step)
}

#[derive(Clone)]
struct EditVersion {
    global: adjustments::Adjustments,
    local: adjustments::Adjustments,
    mask: Option<masking::SelectionMask>,
    raster: Option<image::RgbaImage>,
}

fn adjustment_values(a: adjustments::Adjustments) -> [f32; 12] {
    [
        a.brightness, a.contrast, a.exposure, a.saturation,
        a.temperature, a.tint, a.highlights, a.shadows,
        a.whites, a.blacks, a.vibrance, a.gamma,
    ]
}

fn adjustments_from_values(values: [f32; 12]) -> adjustments::Adjustments {
    adjustments::Adjustments {
        brightness: values[0], contrast: values[1],
        exposure: values[2], saturation: values[3],
        temperature: values[4], tint: values[5],
        highlights: values[6], shadows: values[7],
        whites: values[8], blacks: values[9],
        vibrance: values[10], gamma: values[11],
    }
}

struct PixelForge {
    inactive_tabs: Vec<(usize, Box<PixelForge>)>,
    active_tab: usize,
    next_tab: usize,
    closing_tab: Option<usize>,
    generation_style: usize,
    generation_size: usize,
    generation_unlock_dimensions: bool,
    generation_custom_size: bool,
    generation_resolution: usize,
    generation_dimension_step: u32,
    generation_zoom: f32,
    generation_pan: Vec2,
    generation_batch_count: usize,
    generation_batch_done: usize,
    generation_random_seeds: bool,
    prompt_spelling: spelling::PromptSpelling,
    qwen_steps: u32,
    flux_steps: u32,
    edit_checkpoint: String,
    conversation_draft: String,
    workspace_ai_parent: Option<usize>,
    workspace_editor: Option<usize>,
    crop: Option<crop::Crop>,
    generation_gallery: Vec<GeneratedItem>,
    thumbnail_groups: Vec<ThumbnailGroup>,
    group_dialog: Option<GroupDialog>,
    generation_project_path: Option<PathBuf>,
    refinement_source: Option<usize>,
    refinement_strength: f32,
    refinement_prompt: String,
    refinement_masked: bool,
    refinement_brush: f32,
    refinement_mask: Vec<u8>,
    refinement_mask_texture: Option<TextureHandle>,
    generation_batch_settings: Option<GenerationSettings>,
    generation_selected: usize,
    generation_compare: bool,
    generation_compare_position: f32,
    generation_compare_index: usize,
    rename_version: Option<(usize, String)>,
    generation_workspace: bool,
    generation_prompt: String,
    generation_negative: String,
    generation_auto_negative: bool,
    prompt_guide: prompt_guide::PromptGuide,
    generation_checkpoint: String,
    generation_checkpoints: Vec<String>,
    generation_width: u32,
    generation_height: u32,
    generation_steps: u32,
    generation_cfg: f32,
    generation_seed: u64,
    generation_rx: Option<std::sync::mpsc::Receiver<Result<(image::RgbaImage, u64), String>>>,
    generation_running: bool,
    generation_started: Option<std::time::Instant>,
    generation_progress: std::sync::Arc<std::sync::atomic::AtomicU32>,
    generation_models_rx: Option<std::sync::mpsc::Receiver<Result<Vec<String>, String>>>,
    generation_models_status: String,
    generation_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    generation_image: Option<image::RgbaImage>,
    generation_texture: Option<TextureHandle>,
    generation_status: String,
    install_rx: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    install_model_index: Option<usize>,
    install_progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
    install_total: std::sync::Arc<std::sync::atomic::AtomicU64>,
    install_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    install_status: String,
    dependency_manager: dependencies::Manager,
    updater: updater::Manager,
    remove_model: Option<(usize,PathBuf,Option<String>,bool)>,
    model_library_open: bool,
    install_folder: Option<PathBuf>,
    install_folder_manual: bool,
    ai_rx: Option<std::sync::mpsc::Receiver<Result<ai_upscale::UpscaleResult, String>>>,
    ai_running: bool,
    ai_denoise_level: u32,
    restoration_preset: usize,
    restoration_strength: f32,
    restoration_prompt: String,
    restoration_model: String,
    restoration_steps: u32,
    ai_is_denoise: bool,
    ai_restoration: Option<ai_restoration::Operation>,
    ai_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ai_model: usize,
    ai_gpu_id: i32,
    ai_gpus: Vec<ai_upscale::VulkanGpu>,
    ai_gpu_error: Option<String>,
    ai_tile_size: u32,
    ai_result: Option<std::sync::Arc<ai_upscale::UpscaleResult>>,
    ai_variations: Vec<(String, std::sync::Arc<ai_upscale::UpscaleResult>)>,
    ai_compare_a: usize,
    ai_compare_b: usize,
    ai_save_rx: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    ai_saving: bool,
    ai_strength: f32,
    ai_comparison: bool,
    ai_comparison_position: f32,
    ai_divider_dragging: bool,
    ai_baseline_texture: Option<TextureHandle>,
    ai_status: String,
    ai_result_texture: Option<TextureHandle>,
    ai_result_size: [u32; 2],
    ai_result_path: Option<PathBuf>,
    ai_show_result: bool,
    ai_inspect_texture: Option<TextureHandle>,
    ai_inspect_size: [usize; 2],
    ai_inspect_zoom: f32,
    ai_inspect_pan: Vec2,
    ai_inspect_fit: bool,

    // Native-resolution inspection: one background request at a time.
    ai_detail_rx: Option<
        std::sync::mpsc::Receiver<
            Result<([u32; 4], u32, image::RgbaImage, image::RgbaImage), String>,
        >,
    >,
    ai_detail_key: Option<([u32; 4], u32)>,
    ai_detail_rect: Option<[u32; 4]>,
    ai_detail_ai_texture: Option<TextureHandle>,
    ai_detail_lanczos_texture: Option<TextureHandle>,
    ai_detail_status: String,

    // Cache decoded full-resolution images only when the pair
    // fits within the memory budget.
    ai_decode_cache: std::sync::Arc<
        std::sync::Mutex<
            Option<(
                std::sync::Arc<image::RgbaImage>,
                std::sync::Arc<image::RgbaImage>,
            )>,
        >,
    >,
    selection_mask: Option<masking::SelectionMask>,
    saved_selections: Vec<(String, masking::SelectionMask)>,
    selected_saved_selection: Option<usize>,
    active_saved_selection: Option<usize>,
    selection_name_input: String,
    selection_texture: Option<TextureHandle>,
    selection_brush_enabled: bool,
    selection_show_mask: bool,
    selection_undo: Vec<Option<masking::SelectionMask>>,
    selection_redo: Vec<Option<masking::SelectionMask>>,
    selection_stroke_active: bool,
    selection_erase: bool,
    selection_size: f32,
    selection_hardness: f32,
    selection_opacity: f32,
    selection_last_point: Option<Pos2>,
    texture: Option<TextureHandle>,
    processed_texture: Option<TextureHandle>,
    histogram: histogram::Histogram,
    histogram_mode: histogram::HistogramMode,
    original_pixels: Option<image::RgbaImage>,
    preview_source: Option<std::sync::Arc<image::RgbaImage>>,
    preview_rx: Option<std::sync::mpsc::Receiver<(u64, ColorImage, histogram::Histogram)>>,
    preview_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,

    brightness: f32,
    selection_adjustment_target: bool,
    selection_adjustments: adjustments::Adjustments,
    export_rx: Option<std::sync::mpsc::Receiver<ExportEvent>>,
    export_progress: f32,
    export_stage: String,
    export_encoding: bool,
    exporting: bool,
    export_result: Option<bool>,
    last_export_path: Option<PathBuf>,
    contrast: f32,
    exposure: f32,
    saturation: f32,
    temperature: f32,
    tint: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,
    blacks: f32,
    vibrance: f32,
    gamma: f32,
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
    show_settings: bool,
    blank_project: bool,
    display: display::Display,
    ai_tools_tab: bool,
    generation_tools_tab: bool,
    selection_tools_tab: bool,
    controls: Controls,
    interaction: interaction::Settings,
    status: String,
    project_path: Option<PathBuf>,
    project_dirty: bool,
    edit_versions: Vec<EditVersion>,
    active_raster_version: Option<usize>,
    compare_version_a: usize,
    compare_version_b: usize,
    version_comparison: bool,
    version_texture_a: Option<TextureHandle>,
    version_texture_b: Option<TextureHandle>,
    pending_open: Option<PendingOpen>,
    file_drops: std::collections::VecDeque<PathBuf>,
    exit_confirmation: bool,
    exit_allowed: bool,
}

impl Default for PixelForge {
    fn default() -> Self {
        let model_folder = model_installer::chosen_models_folder();
        let install_folder_manual = model_folder.is_some();
        let (recommended_steps, recommended_cfg, _) = Self::recommended_generation_settings("");
        let (generation_width, generation_height) = generation_dimensions(0, 4, 32);
        Self {
            inactive_tabs: Vec::new(),
            active_tab: 0,
            next_tab: 1,
            closing_tab: None,
            generation_style: 0,
            generation_size: 0,
            generation_unlock_dimensions: false,
            generation_custom_size: false,
            generation_resolution: 4,
            generation_dimension_step: 32,
            generation_zoom: 1.0,
            generation_pan: Vec2::ZERO,
            generation_batch_count: 1,
            generation_batch_done: 0,
            generation_random_seeds: true,
            prompt_spelling: spelling::PromptSpelling::default(),
            qwen_steps: 20,
            flux_steps: 4,
            edit_checkpoint: String::new(),
            conversation_draft: String::new(),
            workspace_ai_parent: None,
            workspace_editor: None,
            crop: None,
            generation_gallery: Vec::new(),
            thumbnail_groups: Vec::new(),
            group_dialog: None,
            generation_project_path: None,
            refinement_source: None,
            refinement_strength: 0.35,
            refinement_prompt: "Remove the extra head on the right. Reconstruct the natural background and preserve the rest of the image.".into(),
            refinement_masked: false,
            refinement_brush: 36.0,
            refinement_mask: Vec::new(),
            refinement_mask_texture: None,
            generation_batch_settings: None,
            generation_selected: 0,
            generation_compare: false,
            generation_compare_position: 0.5,
            generation_compare_index: 0,
            rename_version: None,
            generation_workspace: true,
            generation_prompt: String::new(),
            generation_negative: String::new(),
            generation_auto_negative: false,
            prompt_guide: prompt_guide::PromptGuide::load(),
            generation_checkpoint: String::new(),
            generation_checkpoints: Vec::new(),
            generation_width,
            generation_height,
            generation_steps: recommended_steps,
            generation_cfg: recommended_cfg,
            generation_seed: 42,
            generation_rx: None,
            generation_running: false,
            generation_started: None,
            generation_progress: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            generation_models_rx: None,
            generation_models_status: "Connect to local ComfyUI to begin".into(),
            generation_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            generation_image: None,
            generation_texture: None,
            generation_status: String::new(),
            install_rx: None,
            install_model_index: None,
            install_progress: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            install_total: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            install_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            install_status: String::new(),
            dependency_manager: dependencies::Manager::default(),
            updater: updater::Manager::default(),
            remove_model: None,
            model_library_open: false,
            install_folder: Some(model_folder.unwrap_or_else(model_installer::recommended_models_folder)),
            install_folder_manual,
            ai_rx: None,
            ai_running: false,
            ai_denoise_level: 1,
            restoration_preset: 0,
            restoration_strength: 0.65,
            restoration_prompt: "Restore this photograph. Reduce grain, compression artifacts, and blur. Recover natural-looking detail while preserving the person's identity, facial features, expression, age, skin texture, clothing, composition, and original colors. Do not beautify, change facial proportions, or add objects.".into(),
            restoration_model: ai_generation::FLUX_KLEIN.into(),
            restoration_steps: 4,
            ai_is_denoise: false,
            ai_restoration: None,
            ai_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ai_model: 0,
            ai_gpu_id: -1,
            ai_gpus: Vec::new(),
            ai_gpu_error: None,
            ai_tile_size: 0,
            ai_result: None,
            ai_variations: Vec::new(),
            ai_compare_a: 0,
            ai_compare_b: 0,
            ai_save_rx: None,
            ai_saving: false,
            ai_strength: 1.0,
            ai_comparison: true,
            ai_comparison_position: 0.5,
            ai_divider_dragging: false,
            ai_baseline_texture: None,
            ai_status: String::new(),
            ai_result_texture: None,
            ai_result_size: [0, 0],
            ai_result_path: None,
            ai_show_result: false,
            ai_inspect_texture: None,
            ai_inspect_size: [0, 0],
            ai_inspect_zoom: 1.0,
            ai_inspect_pan: Vec2::ZERO,
            ai_inspect_fit: true,
            ai_detail_rx: None,
            ai_detail_key: None,
            ai_detail_rect: None,
            ai_detail_ai_texture: None,
            ai_detail_lanczos_texture: None,
            ai_detail_status: String::new(),
            ai_decode_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            selection_mask: None,
            saved_selections: Vec::new(),
            selected_saved_selection: None,
            active_saved_selection: None,
            selection_name_input: String::new(),
            selection_texture: None,
            selection_brush_enabled: false,
            selection_show_mask: true,
            selection_undo: Vec::new(),
            selection_redo: Vec::new(),
            selection_stroke_active: false,
            selection_erase: false,
            selection_size: 50.0,
            selection_hardness: 0.7,
            selection_opacity: 1.0,
            selection_last_point: None,
            texture: None,
            processed_texture: None,
            histogram: histogram::Histogram::default(),
            histogram_mode: histogram::HistogramMode::Rgb,
            original_pixels: None,
            preview_source: None,
            preview_rx: None,
            preview_generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),

            brightness: 0.0,
            selection_adjustment_target: false,
            selection_adjustments: adjustments::Adjustments::default(),
            export_rx: None,
            export_progress: 0.0,
            export_stage: String::new(),
            export_encoding: false,
            exporting: false,
            export_result: None,
            last_export_path: None,
            contrast: 0.0,
            exposure: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            vibrance: 0.0,
            gamma: 0.0,
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
            show_settings: false,
            blank_project: false,
            display: display::Display::load(),
            selection_tools_tab: false,
            generation_tools_tab: false,
            ai_tools_tab: profile::settings_root().map(|base|base.join("sidebar_tab.txt"))
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map(|value| value.trim() == "ai")
                .unwrap_or(false),
            controls: Controls::load(),
            interaction: interaction::Settings::load(),
            status: "Ready".into(),
            project_path: None,
            project_dirty: false,
            edit_versions: Vec::new(),
            active_raster_version: None,
            compare_version_a: 0,
            compare_version_b: 0,
            version_comparison: false,
            version_texture_a: None,
            version_texture_b: None,
            pending_open: None,
            file_drops: Default::default(),
            exit_confirmation: false,
            exit_allowed: false,
        }
    }
}

impl PixelForge {
    fn render_edit_version(
        &self,
        ctx: &egui::Context,
        version: &EditVersion,
        name: &str,
    ) -> Option<TextureHandle> {
        let source = self.original_pixels.as_ref()?;
        let mut pixels = version.raster.clone().unwrap_or_else(|| {
            self.edit_versions.iter().position(|entry| std::ptr::eq(entry, version))
                .and_then(|index| self.edit_versions[..index].iter().rev()
                    .find_map(|entry| entry.raster.as_ref()))
                .cloned().unwrap_or_else(|| source.clone())
        });
        let (width, height) = pixels.dimensions();
        for (y, row) in pixels.rows_mut().enumerate() {
            for (x, pixel) in row.enumerate() {
                version.global.apply_pixel(pixel);
                if let Some(mask) = &version.mask {
                    let (mw, mh) = mask.dimensions();
                    if mw > 0 && mh > 0 {
                        let mx = (x as u64 * mw as u64 / width as u64).min(mw as u64 - 1) as u32;
                        let my = (y as u64 * mh as u64 / height as u64).min(mh as u64 - 1) as u32;
                        let alpha = mask.get(mx, my).unwrap_or(0);
                        if alpha > 0 {
                            let before = *pixel;
                            version.local.apply_pixel(pixel);
                            for c in 0..3 {
                                pixel[c] = ((before[c] as u32 * (255 - alpha) as u32
                                    + pixel[c] as u32 * alpha as u32 + 127) / 255) as u8;
                            }
                        }
                    }
                }
            }
        }
        let color = ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize], pixels.as_raw()
        );
        Some(ctx.load_texture(name, color, TextureOptions::LINEAR))
    }

    fn start_version_comparison(&mut self, ctx: &egui::Context) {
        let a = self.edit_versions.get(self.compare_version_a);
        let b = self.edit_versions.get(self.compare_version_b);
        if let (Some(a), Some(b)) = (a, b) {
            let left = self.render_edit_version(ctx, a, "version_compare_left");
            let right = self.render_edit_version(ctx, b, "version_compare_right");
            self.version_comparison = left.is_some() && right.is_some();
            self.version_texture_a = left;
            self.version_texture_b = right;
            self.comparison_enabled = true;
        }
    }

    fn activate_version_source(&mut self, ctx: &egui::Context) {
        let raster = self.active_raster_version
            .and_then(|i| self.edit_versions.get(i))
            .and_then(|v| v.raster.as_ref())
            .or(self.original_pixels.as_ref()).cloned();
        if let Some(raster) = raster {
            let size = [raster.width() as usize, raster.height() as usize];
            self.texture = Some(ctx.load_texture("current_image",
                ColorImage::from_rgba_unmultiplied(size, raster.as_raw()), TextureOptions::LINEAR));
            self.image_size = size;
            self.preview_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.preview_rx = None;
            self.preview_source = Some(std::sync::Arc::new(raster.clone()));
            self.histogram = histogram::Histogram::from_image(&raster);
            self.processed_texture = None;
            self.fit_pending = true;
            self.update_processed_image(ctx);
        }
    }

    fn current_adjustments(&self) -> adjustments::Adjustments {
        adjustments::Adjustments {
            brightness: self.brightness, contrast: self.contrast,
            exposure: self.exposure, saturation: self.saturation,
            temperature: self.temperature, tint: self.tint,
            highlights: self.highlights, shadows: self.shadows,
            whites: self.whites, blacks: self.blacks,
            vibrance: self.vibrance, gamma: self.gamma,
        }
    }

    fn set_current_adjustments(&mut self, a: adjustments::Adjustments) {
        self.brightness = a.brightness;
        self.contrast = a.contrast;
        self.exposure = a.exposure;
        self.saturation = a.saturation;
        self.temperature = a.temperature;
        self.tint = a.tint;
        self.highlights = a.highlights;
        self.shadows = a.shadows;
        self.whites = a.whites;
        self.blacks = a.blacks;
        self.vibrance = a.vibrance;
        self.gamma = a.gamma;
    }

    fn handle_file_drops(&mut self, ctx: &egui::Context) {
        // Keep the queue outside document swaps so multi-file drops retain their order.
        let mut queue = std::mem::take(&mut self.file_drops);
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if !dropped.is_empty() {ctx.request_repaint();}
        for file in dropped {
            if let Some(path) = file.path { queue.push_back(path); }
            else { self.status = "Drop a local image or .pxf project file".into(); }
        }
        while let Some(path) = queue.front().cloned() {
            if self.pending_open.is_some() || self.closing_tab.is_some() || self.exit_confirmation {
                break;
            }
            let extension = path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
            if !path.is_file() || !["pxf","png","jpg","jpeg","webp","bmp","tif","tiff"].contains(&extension.as_str()) {
                self.status = format!("Cannot open {}: drop an image or .pxf project file",path.display());
                self.generation_status = self.status.clone();
                queue.pop_front();
                continue;
            }
            queue.pop_front();
            if extension == "pxf" {
                self.open_project_path(ctx, path);
            } else {
                self.load_image_path(ctx, path);
            }
            self.ensure_workspace(ctx);
        }
        self.file_drops = queue;
    }

    fn open_image(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "tiff"])
            .pick_file()
        else {
            return;
        };

        self.load_image_path(ctx, path);
    }

    fn load_image_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        match image::open(&path) {
            Ok(img) => {
                self.create_project_tab();
                self.blank_project = false;
                self.capture_workspace_editor();
                self.workspace_editor=None;
                self.generation_workspace=false;
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];

                let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

                self.texture =
                    Some(ctx.load_texture("current_image", color_image, TextureOptions::LINEAR));

                self.file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                self.ai_result = None;
                self.ai_variations.clear();
                self.ai_compare_a = 0;
                self.ai_compare_b = 0;
                self.ai_show_result = false;
                self.ai_result_texture = None;
                self.ai_baseline_texture = None;
                self.ai_inspect_texture = None;
                self.ai_result_path = None;
                self.original_pixels = Some(rgba);
                self.edit_versions.clear();
                self.active_raster_version = None;
        self.version_comparison = false;
        self.version_texture_a = None;
        self.version_texture_b = None;
                self.version_comparison = false;
                self.version_texture_a = None;
                self.version_texture_b = None;
                self.prepare_preview_source();
                self.project_path = None;
                self.saved_selections.clear();
                self.selected_saved_selection = None;
                self.active_saved_selection = None;
                self.selection_name_input.clear();
                self.selection_mask = None;
                self.selection_texture = None;
                self.selection_undo.clear();
                self.selection_redo.clear();
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
                self.highlights = 0.0;
                self.shadows = 0.0;
                self.whites = 0.0;
                self.blacks = 0.0;
                self.vibrance = 0.0;
                self.gamma = 0.0;
                self.selection_adjustments = adjustments::Adjustments::default();
                self.selection_adjustment_target = false;
                self.comparison_position = 0.5;
                self.comparison_enabled = false;
                self.divider_dragging = false;
                self.selection_mask = None;
                self.selection_texture = None;
                self.selection_last_point = None;
                self.selection_undo.clear();
                self.selection_redo.clear();
                self.selection_stroke_active = false;
                self.image_size = size;
                self.image_path = Some(path);
                self.fit_pending = true;
                self.offset = Vec2::ZERO;
                self.panning = false;
                self.status = "Image loaded".into();
                self.update_processed_image(ctx);
            }
            Err(err) => {
                self.status = format!("Open failed: {err}");
            }
        }
    }

    fn start_generation(&mut self, ctx: &egui::Context) {
        if self.dependency_manager.busy() {self.generation_status="Wait for dependency setup to finish".into();return;}
        if self.generation_running || self.ai_running {
            return;
        }
        if self.refinement_source.is_some_and(|index|index >= self.generation_gallery.len()) {
            self.refinement_source=None;
            self.generation_status="Refinement source is no longer available".into();
            return;
        }
        let checkpoint = editing_checkpoint(&self.generation_checkpoint, &self.edit_checkpoint, self.refinement_source.is_some()).to_owned();
        self.capture_workspace_editor();
        let style = GENERATION_STYLES[self.generation_style.min(GENERATION_STYLES.len() - 1)].1;
        let steps=if ai_generation::is_qwen(&checkpoint) {self.qwen_steps} else if checkpoint==ai_generation::FLUX_KLEIN && self.refinement_source.is_some() {self.flux_steps} else {self.generation_steps};
        let cfg=if ai_generation::is_qwen(&checkpoint) {4.0} else if ai_generation::ignores_negative(&checkpoint) {1.0} else {self.generation_cfg};
        let prompt = if ai_generation::is_instruction_model(&checkpoint) && self.refinement_source.is_some() {
            self.refinement_prompt.clone()
        } else if self.refinement_source.is_some() {
            // CLIP is not an instruction-following editor: retain scene context,
            // then append the requested correction as guidance.
            let base = self.generation_prompt.trim();
            let edit = self.refinement_prompt.trim();
            if base.is_empty() {edit.to_owned()} else {format!("{base}, {edit}")}
        } else if style.is_empty() {
            self.generation_prompt.clone()
        } else {
            format!("{}, {}", self.generation_prompt, style)
        };
        self.generation_batch_settings = Some(GenerationSettings {
            prompt: self.generation_prompt.clone(),
            negative: if ai_generation::ignores_negative(&checkpoint) {String::new()} else {self.generation_negative.clone()},
            auto_negative: !ai_generation::is_instruction_model(&checkpoint) && !ai_generation::ignores_negative(&checkpoint) && self.generation_auto_negative,
            checkpoint: checkpoint.clone(),
            style: self.generation_style,
            ratio: self.generation_size,
            custom_size: self.generation_custom_size,
            unlock_dimensions: self.generation_unlock_dimensions,
            resolution: self.generation_resolution,
            dimension_step: self.generation_dimension_step,
            width: self.generation_width,
            height: self.generation_height,
            steps,
            cfg,
            seed: self.generation_seed,
            refine_parent: self.refinement_source,
            refine_strength: self.refinement_source.map(|_| self.refinement_strength),
            refine_prompt: self.refinement_source.map(|_| self.refinement_prompt.clone()),
            refine_masked: self.refinement_source.is_some() && self.refinement_masked,
        });
        let request = ai_generation::Request {
            checkpoint: checkpoint.clone(),
            prompt,
            negative: if ai_generation::ignores_negative(&checkpoint) {String::new()} else if !ai_generation::is_instruction_model(&checkpoint) && self.generation_auto_negative {
                let base=automatic_negative_prompt(GENERATION_STYLES[self.generation_style.min(GENERATION_STYLES.len()-1)].0);
                if self.generation_negative.trim().is_empty() {base.to_owned()}
                else {format!("{base}, {}",self.generation_negative.trim())}
            } else {self.generation_negative.clone()},
            width: self.generation_width,
            height: self.generation_height,
            steps,
            cfg,
            seed: self.generation_seed,
            source: self.refinement_source.and_then(|index|self.generation_gallery.get(index)).map(|item|item.editor.render(&item.pixels)),
            denoise: self.refinement_strength,
            mask: if self.refinement_source.is_some() && self.refinement_masked {Some(self.refinement_mask.clone())} else {None},
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.generation_cancel = cancel.clone();
        self.generation_progress = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let progress = self.generation_progress.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.generation_rx = Some(rx);
        self.generation_running = true;
        self.generation_batch_done = 0;
        self.generation_started = Some(std::time::Instant::now());
        self.generation_workspace = true;
        self.generation_status = if self.refinement_source.is_some() {"Refining selected image locally with ComfyUI...".into()} else {"Generating local batch with ComfyUI...".into()};
        let count = if self.refinement_source.is_some() {1} else {self.generation_batch_count};
        let random = if self.refinement_source.is_some() {false} else {self.generation_random_seeds};
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for index in 0..count {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                progress.store(0, std::sync::atomic::Ordering::Relaxed);
                let mut item = request.clone();
                item.seed = if random {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                    now.wrapping_add(index as u64)
                } else {
                    request.seed.wrapping_add(index as u64)
                };
                let seed = item.seed;
                let result = ai_generation::generate(item, cancel.clone(), progress.clone())
                    .map(|image| (image, seed));
                let failed = result.is_err();
                if tx.send(result).is_err() {
                    break;
                }
                ctx.request_repaint();
                if failed {
                    break;
                }
            }
        });
    }

    fn poll_model_catalog(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.install_rx {
            if let Ok(result) = rx.try_recv() {
                self.install_rx = None;
                self.dependency_manager.invalidate();
                match result {
                    Ok(path) => {
                        self.install_status = format!("Installed in {}. Refresh models; restart ComfyUI if Qwen is not yet available.", path.display());
                        self.refresh_generation_models(ctx);
                    }
                    Err(error) => self.install_status = format!("Install failed: {error}"),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        let Some(receiver) = self.generation_models_rx.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(models)) => {
                self.generation_models_rx = None;
                self.generation_models_status = format!("Found {} local checkpoints", models.len());
                self.generation_checkpoints = models;
                self.select_default_checkpoint();
                if !self.install_folder_manual {
                    if let Some(folder) =
                        model_installer::detect_models_folder(&self.generation_checkpoints)
                    {
                        self.install_folder = Some(folder);
                    }
                }
            }
            Ok(Err(error)) => {
                self.generation_models_rx = None;
                self.generation_models_status = error;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.generation_models_rx = None;
                self.generation_models_status = "The model discovery worker stopped. Reconnect to try again.".into();
            }
        }
    }

    fn select_default_checkpoint(&mut self) {
        if self.generation_checkpoint.is_empty() {
            if let Some(checkpoint) = self.generation_checkpoints.first() {
                self.generation_checkpoint = checkpoint.clone();
                self.reset_generation_sampling();
            }
        }
    }

    fn poll_generation(&mut self, ctx: &egui::Context) {
        if !self.generation_running {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        let received = self.generation_rx.as_ref().map(|rx| rx.try_recv());
        match received {
            Some(Ok(Ok((image, seed)))) => {
                let size = [image.width() as usize, image.height() as usize];
                let preview = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                let texture = ctx.load_texture(
                    format!("generation_{}_{}", self.generation_gallery.len(), seed),
                    preview,
                    TextureOptions::LINEAR,
                );
                self.generation_texture = Some(texture.clone());
                self.generation_image = Some(image.clone());
                let mut settings = self.generation_batch_settings.clone()
                    .expect("running batch has generation settings");
                settings.seed = seed;
                settings.width = image.width();
                settings.height = image.height();
                self.blank_project = false;
                self.generation_gallery.push(GeneratedItem {
                    group: None,
                    editor: studio::EditorState::default(),
                    settings,
                    pixels: image,
                    texture,
                    seed,
                });
                self.generation_selected = self.generation_gallery.len() - 1;
                self.project_dirty=true;
                self.generation_batch_done += 1;
                self.generation_status = format!(
                    "Generated {} of {} images",
                    self.generation_batch_done, self.generation_batch_count
                );
                if self.generation_batch_done >= (if self.generation_batch_settings.as_ref().and_then(|s|s.refine_parent).is_some(){1}else{self.generation_batch_count}) {
                    self.generation_running = false;
                    self.generation_rx = None;
                    let elapsed = self
                        .generation_started
                        .take()
                        .map(|t| t.elapsed().as_secs_f32())
                        .unwrap_or(0.0);
                    self.generation_status = format!("Batch complete in {elapsed:.1}s.");
                }
            }
            Some(Ok(Err(error))) => {
                self.generation_running = false;
                self.generation_rx = None;
                self.generation_started = None;
                self.generation_status = error;
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.generation_running = false;
                self.generation_rx = None;
                self.generation_started = None;
                self.generation_status = if self
                    .generation_cancel
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    "Batch cancelled.".into()
                } else {
                    "Generation worker finished.".into()
                };
            }
            _ => {}
        }
    }

    fn refresh_generation_models(&mut self, ctx: &egui::Context) {
        if self.dependency_manager.busy() {self.generation_models_status="Wait for the dependency check or setup to finish, then refresh models".into();return;}
        if self.generation_models_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.generation_models_rx = Some(rx);
        self.generation_models_status =
            "Connecting to local ComfyUI (starting backend if needed)...".into();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ai_generation::checkpoints());
            repaint.request_repaint();
        });
    }

    fn recommended_generation_settings(checkpoint: &str) -> (u32, f32, &'static str) {
        let name = checkpoint.to_ascii_lowercase();
        if checkpoint==ai_generation::FLUX_KLEIN {
            (4,1.0,"FLUX Klein distilled: 4 steps, CFG fixed at 1. Negative prompts are unused.")
        } else if checkpoint==ai_generation::Z_IMAGE {
            (9,1.0,"Z-Image Turbo: start at 9 steps, CFG fixed at 1. Negative prompts are unused.")
        } else if name.contains("cyberrealisticpony") || name.contains("pony") {
            (30, 5.0, "Pony-based checkpoint: start at 30 steps, CFG 5.")
        } else if name.contains("cyberrealisticxl") {
            (30, 4.0, "CyberRealistic XL v10: start at 30 steps, CFG 4 (recommended range 3–5).")
        } else if name.contains("realvis") {
            (30, 5.0, "RealVisXL: start at 30 steps, CFG 5; adjust for the checkpoint version.")
        } else if name.contains("juggernaut") {
            (30, 5.0, "Juggernaut XL: start at 30 steps, CFG 5; adjust for the checkpoint version.")
        } else if name.contains("sd_xl_base") || name.contains("sdxl_base") {
            (30, 7.0, "SDXL Base: start at 30 steps, CFG 7.")
        } else {
            (30, 6.0, "Unknown checkpoint: conservative SDXL starting point, 30 steps and CFG 6.")
        }
    }

    fn reset_generation_sampling(&mut self) {
        let (steps, cfg, _) = Self::recommended_generation_settings(&self.generation_checkpoint);
        self.generation_steps = steps;
        self.generation_cfg = cfg;
    }

    fn capture_workspace_editor(&mut self) {
        let Some(index) = self.workspace_editor else {return};
        let current = self.current_adjustments();
        // The slider UI restores these banks before the shared canvas/save path.
        let (global,local) = (current,self.selection_adjustments);
        if let Some(item) = self.generation_gallery.get_mut(index) {
            let editor = studio::EditorState {name:item.editor.name.clone(),global,local,mask:self.selection_mask.clone(),saved_selections:self.saved_selections.clone()};
            if item.editor!=editor {self.project_dirty=true;}
            item.editor=editor;
            if let Some(texture)=&self.processed_texture {item.texture=texture.clone();}
        }
    }

    fn bind_workspace_editor(&mut self, ctx: &egui::Context) {
        if self.workspace_editor == Some(self.generation_selected) {return;}
        self.crop=None;
        self.capture_workspace_editor();
        let Some(item) = self.generation_gallery.get(self.generation_selected) else {return};
        let pixels = item.pixels.clone();
        let editor = item.editor.clone();
        self.processed_texture = Some(item.texture.clone());
        self.texture = Some(ctx.load_texture("studio_original",ColorImage::from_rgba_unmultiplied([pixels.width() as usize,pixels.height() as usize],pixels.as_raw()),TextureOptions::LINEAR));
        self.image_size = [pixels.width() as usize,pixels.height() as usize];
        self.original_pixels = Some(pixels);
        self.workspace_editor = Some(self.generation_selected);
        self.set_current_adjustments(editor.global);
        self.selection_adjustments = editor.local;
        self.selection_adjustment_target = false;
        self.selection_mask = editor.mask;
        self.saved_selections = editor.saved_selections;
        self.selected_saved_selection = None;
        self.active_saved_selection = None;
        self.selection_undo.clear();
        self.selection_redo.clear();
        self.selection_last_point = None;
        self.selection_stroke_active = false;
        self.edit_versions.clear();
        self.active_raster_version = None;
        self.version_comparison = false;
        self.version_texture_a = None;
        self.version_texture_b = None;
        self.comparison_enabled = false;
        self.ai_show_result = false;
        self.prepare_preview_source();
        self.update_processed_image(ctx);
        self.refresh_selection_overlay(ctx);
    }

    fn ensure_workspace(&mut self, ctx: &egui::Context) {
        if self.blank_project { return; }
        if !self.generation_workspace && self.workspace_editor.is_none() {
            if let Some(pixels) = self.preview_source.as_ref().map(|p|p.as_ref().clone()).or_else(||self.original_pixels.clone()) {
                let mut settings = settings_from_json(&serde_json::json!({"checkpoint":"Imported image","width":pixels.width(),"height":pixels.height()}));
                settings.custom_size=true;
                let texture=self.texture.as_ref().expect("loaded editor has texture").clone();
                self.blank_project=false;self.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings,pixels,texture,seed:0});
                self.generation_selected=self.generation_gallery.len()-1;
                self.workspace_editor=Some(self.generation_selected);
                self.capture_workspace_editor();
            }
        }
        if !self.generation_gallery.is_empty() {self.generation_workspace=true;self.bind_workspace_editor(ctx);}
    }

    fn show_edit_sidebar(&mut self,ui:&mut egui::Ui,ctx:&egui::Context) {
        theme::collapsible_section(ui,"Histogram",|ui|self.show_histogram(ui));
        ui.add_space(8.0);
        theme::collapsible_section(ui,"Crop",|ui|self.show_crop_controls(ui,ctx));
        ui.add_space(8.0);
        theme::collapsible_section(ui,"Image Adjustments",|ui| {
            crate::interaction::strong(ui,"Auto Enhance");
            if ui.add_enabled(!self.exporting,egui::Button::new("Auto Enhance Image")).clicked() {
                self.run_auto_enhance(ctx);
            }
            crate::interaction::label(ui,"Automatic exposure, tonal and color correction. Fine-tune the results with the sliders below.");
            ui.separator();
            self.show_adjustments(ui);
        });
        ui.add_space(8.0);
        theme::collapsible_section(ui,"Image Analysis",|ui|self.show_image_analysis(ui));
        ui.add_space(12.0);
    }

    fn show_crop_controls(&mut self,ui:&mut egui::Ui,ctx:&egui::Context) {
        let available=!self.generation_running && !self.ai_running && !self.exporting && !self.ai_saving;
        if self.crop.is_none() {
            if ui.add_enabled(available && !self.generation_gallery.is_empty(),egui::Button::new("Crop Image")).clicked() {
                self.capture_workspace_editor();
                self.generation_compare=false;
                self.selection_brush_enabled=false;
                self.crop=Some(crop::Crop::new(self.generation_selected,[self.image_size[0] as u32,self.image_size[1] as u32]));
            }
            return;
        }
        let size=[self.image_size[0] as u32,self.image_size[1] as u32];
        let crop=self.crop.as_mut().unwrap();
        if ui.checkbox(&mut crop.locked,"Lock aspect ratio").changed() && crop.locked {crop.lock_current();}
        let mut selected=None;
        egui::ComboBox::from_id_salt("crop_ratio").width(ui.available_width()).truncate()
            .selected_text(if !crop.locked {"Free aspect ratio"} else {
                let (_,w,h)=GENERATION_SIZES[crop.preset];
                if (crop.locked_ratio()-w as f32/h as f32).abs()<0.001 {GENERATION_SIZES[crop.preset].0}else{"Custom locked ratio"}
            }).show_ui(ui,|ui| {
                for (index,&(label,_,_)) in GENERATION_SIZES.iter().enumerate() {
                    if ui.selectable_label(crop.preset==index,label).clicked() {selected=Some(index);}
                }
            });
        if let Some(index)=selected {
            crop.preset=index;
            let (_,w,h)=GENERATION_SIZES[index];
            crop.set_aspect(w as f32/h as f32,size);
        }
        let [_,_,mut width,mut height]=crop.pixels(size);
        let mut width_changed=false;
        let mut height_changed=false;
        ui.horizontal(|ui| {
            crate::interaction::label(ui,"Crop Width");
            width_changed=ui.add(numeric::DragValue::new(&mut width).range(1..=size[0]).speed(1).suffix(" px").update_while_editing(false)).changed();
        });
        ui.horizontal(|ui| {
            crate::interaction::label(ui,"Crop Height");
            height_changed=ui.add(numeric::DragValue::new(&mut height).range(1..=size[1]).speed(1).suffix(" px").update_while_editing(false)).changed();
        });
        if width_changed || height_changed {crop.set_pixel_size(width,height,width_changed,size);}
        crate::interaction::small(ui,"Click a size to type an exact pixel value, or drag the number to adjust.");
        crate::interaction::label(ui,"Zoom mode");
        egui::ComboBox::from_id_salt("crop_zoom_mode").width(ui.available_width()).truncate()
            .selected_text(match crop.zoom_mode {crop::ZoomMode::Workspace=>"Zoom image and crop together",crop::ZoomMode::ImageOnly=>"Zoom image inside crop"})
            .show_ui(ui,|ui| {
                ui.selectable_value(&mut crop.zoom_mode,crop::ZoomMode::Workspace,"Zoom image and crop together");
                ui.selectable_value(&mut crop.zoom_mode,crop::ZoomMode::ImageOnly,"Zoom image inside crop");
            });
        crate::interaction::small(ui,"Scroll over the image or use + / − to zoom. Drag the crop's corners or edges to resize; drag inside to move it.");
        crate::interaction::small(ui,match crop.zoom_mode {
            crop::ZoomMode::Workspace=>"Image and crop zoom together without changing crop pixels. Middle-drag pans the whole view.",
            crop::ZoomMode::ImageOnly=>"The frame stays in place while the image zooms inside it. Middle-drag repositions the image within the frame.",
        });
        let mut apply=false;let mut cancel=false;
        ui.horizontal(|ui| {
            apply=ui.add_enabled(available,theme::primary("Apply Crop")).clicked();
            cancel=ui.button("Cancel Crop").clicked();
        });
        if apply {self.apply_workspace_crop(ctx);} else if cancel {self.crop=None;}
    }
    fn apply_workspace_crop(&mut self,ctx:&egui::Context) {
        if self.generation_running || self.ai_running || self.exporting || self.ai_saving {return;}
        let Some(crop)=self.crop.take() else {return};
        if crop.source!=self.generation_selected {return;}
        self.capture_workspace_editor();
        let parent=self.generation_selected;
        let Some(item)=self.generation_gallery.get(parent) else {return};
        let [x,y,w,h]=crop.pixels([item.pixels.width(),item.pixels.height()]);
        let pixels=image::imageops::crop_imm(&item.pixels,x,y,w,h).to_image();
        let crop_mask=|mask:&masking::SelectionMask| {
            let (mw,mh)=mask.dimensions();
            let gray=image::GrayImage::from_raw(mw,mh,mask.pixels().to_vec()).expect("valid selection mask");
            let gray=image::imageops::crop_imm(&gray,x,y,w,h).to_image();
            masking::SelectionMask::from_pixels(w,h,gray.into_raw()).expect("matching cropped mask")
        };
        let mut editor=item.editor.clone();
        editor.name=format!("Crop {w} × {h}");
        editor.mask=editor.mask.as_ref().map(&crop_mask);
        for (_,mask) in &mut editor.saved_selections {*mask=crop_mask(mask);}
        let rendered=editor.render(&pixels);
        let texture=ctx.load_texture(format!("studio_crop_{}",self.generation_gallery.len()),ColorImage::from_rgba_unmultiplied([w as usize,h as usize],rendered.as_raw()),TextureOptions::LINEAR);
        let mut settings=item.settings.clone();
        settings.width=w;settings.height=h;settings.custom_size=true;
        settings.refine_parent=Some(parent);settings.refine_prompt=Some(format!("Crop at {x}, {y}: {w} × {h}"));
        let seed=item.seed;
        self.blank_project=false;self.generation_gallery.push(GeneratedItem {group:None, editor,settings,pixels,texture,seed});
        self.generation_selected=self.generation_gallery.len()-1;
        self.project_dirty=true;
        self.refinement_source=None;
        self.refinement_mask.clear();self.refinement_mask_texture=None;
        self.generation_zoom=1.0;self.generation_pan=Vec2::ZERO;
        self.bind_workspace_editor(ctx);
        self.generation_status=format!("Created cropped version: {w} × {h}. Original retained.");
    }

    fn keep_workspace_version(&mut self, ctx: &egui::Context, label: &str) {
        self.capture_workspace_editor();
        let Some(item)=self.generation_gallery.get(self.generation_selected) else {return};
        let mut settings=item.settings.clone();
        let pixels=item.editor.render(&item.pixels);
        settings.refine_parent=Some(self.generation_selected);
        settings.refine_prompt=Some(label.into());
        let texture=ctx.load_texture(format!("studio_snapshot_{}",self.generation_gallery.len()),ColorImage::from_rgba_unmultiplied([pixels.width() as usize,pixels.height() as usize],pixels.as_raw()),TextureOptions::LINEAR);
        let seed=item.seed;
        self.blank_project=false;self.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings,pixels,texture,seed});
        self.generation_selected=self.generation_gallery.len()-1;
        self.project_dirty=true;
        self.bind_workspace_editor(ctx);
    }

    fn rotate_workspace_version(&mut self, ctx: &egui::Context, clockwise: bool) {
        if self.generation_running || self.ai_running || self.exporting || self.ai_saving { return; }
        self.bind_workspace_editor(ctx);
        self.capture_workspace_editor();
        let parent = self.generation_selected;
        let Some(item) = self.generation_gallery.get(parent) else { return; };
        let mut editor = item.editor.clone();
        let pixels = if clockwise { image::imageops::rotate90(&item.pixels) } else { image::imageops::rotate270(&item.pixels) };
        let rotate_mask = |mask: &masking::SelectionMask| {
            let (w,h) = mask.dimensions();
            let source = image::GrayImage::from_raw(w,h,mask.pixels().to_vec()).expect("valid selection mask");
            let rotated = if clockwise { image::imageops::rotate90(&source) } else { image::imageops::rotate270(&source) };
            masking::SelectionMask::from_pixels(rotated.width(),rotated.height(),rotated.into_raw()).expect("rotated mask has matching dimensions")
        };
        editor.mask = editor.mask.as_ref().map(&rotate_mask);
        for (_,mask) in &mut editor.saved_selections { *mask = rotate_mask(mask); }
        let display = editor.render(&pixels);
        let texture = ctx.load_texture(format!("studio_rotation_{parent}"), ColorImage::from_rgba_unmultiplied([pixels.width() as usize, pixels.height() as usize], display.as_raw()), TextureOptions::LINEAR);
        let item = &mut self.generation_gallery[parent];
        item.settings.width = pixels.width();
        item.settings.height = pixels.height();
        item.pixels = pixels;
        item.editor = editor;
        item.texture = texture;
        // Reload this version without capturing the old, unrotated editor state.
        self.workspace_editor = None;
        self.refinement_source = None;
        self.refinement_mask.clear();
        self.refinement_mask_texture = None;
        self.ai_result = None;
        self.ai_result_texture = None;
        self.project_dirty = true;
        self.bind_workspace_editor(ctx);
    }

    fn append_ai_workspace_result(&mut self, ctx: &egui::Context, label: &str) -> Result<(),String> {
        let result=self.ai_result.clone().ok_or("No AI result is available")?;
        let ai=load_inspection_png(&result.ai_path,"AI result")?;
        let pixels=if self.ai_strength>=1.0 {ai} else {
            let baseline=load_inspection_png(&result.baseline_path,"AI baseline")?;
            ai_upscale::blend_images_owned(&ai,baseline,self.ai_strength)?
        };
        self.capture_workspace_editor();
        let parent=self.workspace_ai_parent.unwrap_or(self.generation_selected);
        let source=self.generation_gallery.get(parent).ok_or("AI source version is no longer available")?;
        let mut settings=source.settings.clone();
        settings.refine_parent=Some(parent);settings.refine_prompt=Some(format!("{label} · strength {:.0}%",self.ai_strength*100.0));
        settings.checkpoint=format!("AI Tools: {label}");settings.width=pixels.width();settings.height=pixels.height();settings.steps=0;settings.cfg=0.0;
        let texture=ctx.load_texture(format!("studio_ai_{}",self.generation_gallery.len()),ColorImage::from_rgba_unmultiplied([pixels.width() as usize,pixels.height() as usize],pixels.as_raw()),TextureOptions::LINEAR);
        self.blank_project=false;self.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings,pixels,texture,seed:0});
        self.generation_selected=self.generation_gallery.len()-1;
        self.project_dirty=true;
        self.bind_workspace_editor(ctx);
        self.ai_show_result=false;
        Ok(())
    }

    fn show_generation_sidebar(&mut self, ui: &mut egui::Ui) {
        if !self.generation_workspace {
            if ui.button("Open Image Studio").clicked() { self.generation_workspace = true; }
        }
        if self.generation_models_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                crate::interaction::label(ui,"Loading available models…");
            });
            crate::interaction::label(ui,"Connecting to ComfyUI; the first connection may take a moment.");
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            ui.separator();
        }
        if !self.generation_models_status.is_empty() {
            crate::interaction::label(ui, &self.generation_models_status);
        }
        self.show_conversation(ui);
        ui.separator();
        if self.generation_checkpoint.is_empty() {
            crate::interaction::label(ui,"Connect a local model to begin.");
            if ui.button("Connect Models").clicked() { self.refresh_generation_models(ui.ctx()); }
        }
        if self
            .generation_status
            .contains(ai_generation::CPU_FALLBACK_HINT)
            && ui
                .add_enabled(
                    self.generation_models_rx.is_none()
                        && !self.dependency_manager.busy()
                        && !self.backend_jobs_running()
                        && !self
                            .inactive_tabs
                            .iter()
                            .any(|(_, tab)| tab.backend_jobs_running()),
                    egui::Button::new("Try with CPU"),
                )
                .clicked()
        {
            match self.dependency_manager.try_cpu() {
                Ok(()) => self.refresh_generation_models(ui.ctx()),
                Err(error) => self.generation_status = error,
            }
        }
        if ui.button("Open Model Library").clicked() { self.model_library_open = true; }
    }

    fn show_model_library(&mut self, ctx: &egui::Context) {
        if !self.model_library_open { return; }
        let mut open = self.model_library_open;
        egui::Window::new("Model Library")
            .collapsible(false)
            .open(&mut open)
            .default_width(650.0)
            .default_height(700.0)
            .vscroll(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ctx, |ui| {
            ui.heading("PixelForge model library");
            theme::section(ui, "AI Assist");
            if self.prompt_guide.show_ollama_library(ui, &self.generation_checkpoint) {
                self.dependency_manager.open = true;
            }
            ui.separator();
            crate::interaction::label(ui,"ComfyUI models");
            if let Some(folder)=&self.install_folder {
                crate::interaction::label(ui,format!("Models folder: {}",folder.display()));
            } else {
                crate::interaction::label(ui,"Choose the models folder used by ComfyUI.");
            }
            if ui.add_enabled(self.install_rx.is_none(),egui::Button::new("Choose Models Folder…")).clicked() {
                if let Some(folder)=rfd::FileDialog::new().set_title("Select the ComfyUI models folder (contains checkpoints, diffusion_models, text_encoders, vae)").pick_folder() {
                    // Also accept the old checkpoints-folder choice.
                    let folder = if folder.file_name().is_some_and(|name| name == "checkpoints") {
                        folder.parent().unwrap_or(&folder).to_path_buf()
                    } else {
                        folder
                    };
                    match model_installer::save_models_folder(&folder) {
                        Ok(()) => {
                            self.install_folder = Some(folder);
                            self.install_folder_manual = true;
                            self.dependency_manager.invalidate();
                            self.install_status = "Models folder saved. Reconnect the managed backend to use it.".into();
                        }
                        Err(error) => self.install_status = format!("Cannot save models folder: {error}"),
                    }
                }
            }
            for (heading, generation, editing) in [
                ("Image Generation", true, false),
                ("Generation and Editing", true, true),
                ("Instruction Editing", false, true),
            ] {
                ui.add_space(8.0);
                theme::section(ui, heading);
                for (index, model) in model_installer::MODELS.iter().enumerate()
                    .filter(|(_, model)| model.generation == generation && model.editing == editing)
                {
                    let present=self.install_folder.as_ref().map(|root|model_installer::installed_files(model,root)).unwrap_or(0);
                    let backend_model=(!model.bundle()).then(||self.generation_checkpoints.iter().find(|name|name.to_lowercase().contains(model.keyword)).cloned()).flatten();
                    let installed=present==model.files.len() || backend_model.is_some();
                    ui.group(|ui| {
                        // Use the whole column instead of sizing each card to its text.
                        ui.set_width(ui.available_width());
                        ui.heading(egui::RichText::new(model.name).size(23.0).strong());
                        crate::interaction::label(ui,if installed {"Installed".to_owned()} else if present>0 {format!("{} / {} files installed",present,model.files.len())} else {"Not installed".to_owned()});
                        crate::interaction::label(ui,model.description);
                        model.show_compatibility(ui);
                        ui.hyperlink_to("Model page and license",model.page);
                        if installed {
                            if model.generation && ui.button("Use for Generation").clicked() {
                                self.generation_checkpoint=if model.bundle() {model.name.into()} else {backend_model.clone().unwrap_or_else(||model.files[0].file.to_owned())};
                                self.reset_generation_sampling();
                            }
                            if model.editing && ui.button("Use for Edits").clicked() {
                                self.edit_checkpoint=model.name.into();
                                if model.name==ai_generation::QWEN_EDIT_2511 {self.qwen_steps=40;}
                            }
                        } else if ui.add_enabled(self.install_rx.is_none()&&!self.dependency_manager.busy(),egui::Button::new("Download & Install")).clicked() {
                            let folder=self.install_folder.clone().or_else(||rfd::FileDialog::new().set_title("Select the ComfyUI models folder").pick_folder());
                            if let Some(folder)=folder {
                                let folder=if folder.file_name().is_some_and(|n|n=="checkpoints") {folder.parent().unwrap_or(&folder).to_path_buf()} else {folder};
                                self.install_folder=Some(folder.clone());
                                self.install_cancel.store(false,std::sync::atomic::Ordering::Relaxed);
                                self.install_progress.store(0,std::sync::atomic::Ordering::Relaxed);
                                self.install_total.store(0,std::sync::atomic::Ordering::Relaxed);
                                let progress=self.install_progress.clone();let total=self.install_total.clone();let cancel=self.install_cancel.clone();
                                let (tx,rx)=std::sync::mpsc::channel();self.install_rx=Some(rx);
                                self.install_model_index=Some(index);
                                self.install_status=format!("Downloading {}…",model.name);
                                let repaint=ui.ctx().clone();
                                std::thread::spawn(move || {
                                    let _=tx.send(model_installer::install(index,folder,progress,total,cancel));
                                    repaint.request_repaint();
                                });
                            }
                        }
                        if present>0 || backend_model.is_some() {
                            if ui.add_enabled(!self.generation_running&&!self.ai_running&&self.install_rx.is_none()&&!self.dependency_manager.busy(),egui::Button::new("Remove Model…")).clicked() {
                                if let Some(root)=&self.install_folder {self.remove_model=Some((index,root.clone(),backend_model.clone(),false));}
                                else {self.install_model_index=None;self.install_status="Choose the model folder before removing files".into();}
                            }
                        }
                        if self.install_model_index==Some(index) {
                            if self.install_rx.is_some() {
                                let bytes=self.install_progress.load(std::sync::atomic::Ordering::Relaxed);
                                let total=self.install_total.load(std::sync::atomic::Ordering::Relaxed);
                                ui.add(egui::ProgressBar::new(if total>0 {bytes as f32/total as f32} else {0.0})
                                    .text(if total>0 {format!("{:.2} / {:.2} GB",bytes as f64/1e9,total as f64/1e9)} else {"Preparing download…".into()}));
                                if ui.button("Cancel Download").clicked() {self.install_cancel.store(true,std::sync::atomic::Ordering::Relaxed);}
                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                            }
                            if !self.install_status.is_empty() {crate::interaction::label(ui,&self.install_status);}
                        }
                    });
                }
            }
            ui.add_space(8.0);
            theme::section(ui, "Restoration and Upscaling");
            for (component, compatibility) in [
                (dependencies::Component::Denoise, "Requires a compatible Vulkan GPU. CPU processing is unavailable in this engine."),
                (dependencies::Component::Upscale, "Requires a compatible Vulkan GPU. CPU processing is unavailable in this engine."),
                (dependencies::Component::DeblurModel, "Uses the ONNX Runtime CPU provider. Requires the Deblur Python runtime."),
            ] {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.heading(egui::RichText::new(component.title()).size(23.0).strong());
                    if let Some(row) = self.dependency_manager.rows.iter().find(|row| row.component == Some(component)) {
                        ui.label(if row.installed { "Installed" } else { "Not installed" });
                    }
                    ui.label(compatibility);
                });
            }
            if ui.button("Open restoration and upscaling setup").clicked() {
                self.dependency_manager.open = true;
            }
            if self.install_model_index.is_none() && !self.install_status.is_empty() {crate::interaction::label(ui,&self.install_status);}
            crate::interaction::label(ui,"Existing complete files are kept. Refresh models after installation; Qwen may need a ComfyUI restart.");
            if !self.generation_models_status.is_empty() {
                crate::interaction::label(ui, &self.generation_models_status);
            }
            if ui.add_enabled(self.generation_models_rx.is_none(),egui::Button::new("Refresh Installed Models")).clicked() {self.refresh_generation_models(ui.ctx());}
        });
        self.model_library_open = open;
    }

    fn show_model_removal(&mut self,ctx:&egui::Context) {
        let Some((index,root,backend,mut shared))=self.remove_model.clone() else {return};
        let model=&model_installer::MODELS[index];let mut open=true;let mut confirm=false;let mut cancel=false;
        egui::Window::new("Remove Model").open(&mut open).collapsible(false).show(ctx,|ui| {
            ui.heading(model.name);
            if model.bundle() {ui.checkbox(&mut shared,"Also remove the shared text encoder and VAE");crate::interaction::label(ui,"Shared files may be used by other models. Leave this unchecked to keep them.");}
            match model_installer::removal_plan(index,&root,backend.as_deref(),shared) {
                Ok(files)=>{
                    let bytes:u64=files.iter().map(|f|f.bytes).sum();crate::interaction::label(ui,format!("Free {:.2} GB by permanently deleting:",bytes as f64/1e9));
                    for file in &files {crate::interaction::label(ui,file.path.display().to_string());}
                    confirm=ui.add_enabled(!files.is_empty()&&!self.generation_running&&!self.ai_running&&self.install_rx.is_none(),egui::Button::new("Remove These Files")).clicked();
                }
                Err(error)=>{crate::interaction::label(ui,error);}
            }
            if ui.button("Cancel").clicked() {cancel=true;}
        });
        if confirm {
            ai_generation::stop_managed_backend();
            self.install_model_index=Some(index);
            self.install_status=match model_installer::remove_model_files(index,&root,backend.as_deref(),shared) {
                Ok(bytes)=>{
                    if model.bundle() {
                        if self.edit_checkpoint==model.name {self.edit_checkpoint=String::new();}
                        if self.generation_checkpoint==model.name {self.generation_checkpoint=self.generation_checkpoints.first().cloned().unwrap_or_default(); self.reset_generation_sampling();}
                    }
                    else if let Some(name)=&backend {self.generation_checkpoints.retain(|s|s!=name);if self.generation_checkpoint==*name {self.generation_checkpoint=self.generation_checkpoints.first().cloned().unwrap_or_default(); self.reset_generation_sampling();}}
                    self.dependency_manager.refresh(self.install_folder.clone(),self.generation_checkpoints.clone());
                    format!("Removed model files; freed {:.2} GB. Restart ComfyUI to release any loaded model memory.",bytes as f64/1e9)
                }
                Err(error)=>format!("Model removal: {error}"),
            };self.remove_model=None;
        } else if !open||cancel {self.remove_model=None;} else {self.remove_model=Some((index,root,backend,shared));}
    }

    fn show_generation_settings(&mut self,ui:&mut egui::Ui) {
        let ctx=ui.ctx().clone();
        ui.spacing_mut().slider_width=(ui.available_width()-80.0).max(60.0);
        ui.checkbox(&mut self.prompt_spelling.enabled,"Check English spelling");
        crate::interaction::label(ui,"Local GPU generation via ComfyUI (127.0.0.1:8188)");
        if ui.button("Connect / Refresh Models").clicked() {
            self.refresh_generation_models(&ctx);
        }
        let (_, _, recommendation) =
            Self::recommended_generation_settings(&self.generation_checkpoint);
        crate::interaction::small(ui,recommendation);
        let selected = self.generation_checkpoint.to_ascii_lowercase();
        let best_for = if self.generation_checkpoint==ai_generation::Z_IMAGE {
            "Best for: Fast photographic generation, people, natural lighting, and texture."
        } else if selected.contains("cyberrealisticxl") {
            "Best for: Photographic people, natural skin detail, portraits, and cinematic lighting."
        } else if selected.contains("realvis") {
            "Best for: Natural-looking photography, portraits, landscapes, and everyday scenes."
        } else if selected.contains("juggernaut") {
            "Best for: Highly detailed images, cinematic photography, dramatic lighting, and polished compositions."
        } else if selected.contains("sd_xl_base") || selected.contains("sdxl_base") {
            "Best for: General-purpose generation, creative experimentation, illustrations, and prompt testing."
        } else if selected.is_empty() {
            "Connect to ComfyUI and select a model to see its strengths."
        } else {
            "Custom model: Results depend on the checkpoint's training and recommended settings."
        };
        crate::interaction::small(ui,best_for);
        ui.separator();
        let previous_ratio = self.generation_size;
        crate::interaction::label(ui,"Style preset");
        egui::ComboBox::from_id_salt("sidebar_Style_preset").width(ui.available_width()).truncate()
            .selected_text(
                GENERATION_STYLES[self.generation_style.min(GENERATION_STYLES.len() - 1)].0,
            )
            .show_ui(ui, |ui| {
                for (i, (name, _)) in GENERATION_STYLES.iter().enumerate() {
                    ui.selectable_value(&mut self.generation_style, i, *name);
                }
            });
        if selected.contains("cyberrealisticpony") {
            crate::interaction::small(ui, "Style presets add prompt text. CyberRealistic Pony can still favor a photographic look, even with an illustration preset.");
        }

        crate::interaction::label(ui,"Aspect ratio");
        egui::ComboBox::from_id_salt("sidebar_Aspect_ratio").width(ui.available_width()).truncate()
            .selected_text(GENERATION_SIZES[self.generation_size.min(GENERATION_SIZES.len() - 1)].0)
            .show_ui(ui, |ui| {
                for (i, (name, _, _)) in GENERATION_SIZES.iter().enumerate() {
                    ui.selectable_value(&mut self.generation_size, i, *name);
                }
            });

        crate::interaction::label(ui,"Dimension rounding");
        let previous_step=self.generation_dimension_step;
        egui::ComboBox::from_id_salt("dimension_rounding").width(ui.available_width()).selected_text(format!("{} pixels",self.generation_dimension_step)).show_ui(ui,|ui| {
            for step in [16,32,64] {ui.selectable_value(&mut self.generation_dimension_step,step,format!("{step} pixels"));}
        });
        // IDs 0–3 retain their old serialized meanings; Medium is ID 4.
        let presets = [(0,"Small"),(4,"Medium"),(1,"Standard"),(2,"Large"),(3,"Extra Large")];
        let resolution_labels: Vec<(usize,String)> = presets.iter().map(|&(id,name)| {
            let (w,h)=generation_dimensions(self.generation_size,id,self.generation_dimension_step);
            (id,format!("{name} — {w} × {h}"))
        }).collect();
        crate::interaction::label(ui,"Resolution");
        egui::ComboBox::from_id_salt("sidebar_Resolution").width(ui.available_width()).truncate()
            .selected_text(&resolution_labels.iter().find(|(id,_)|*id==self.generation_resolution).unwrap_or(&resolution_labels[0]).1)
            .show_ui(ui, |ui| {
                for (id,label) in &resolution_labels {ui.selectable_value(&mut self.generation_resolution,*id,label);}
            });

        let custom_changed = ui
            .checkbox(&mut self.generation_custom_size, "Custom resolution")
            .changed();

        let (preset_w, preset_h) =
            generation_dimensions(self.generation_size, self.generation_resolution, self.generation_dimension_step);

        if !self.generation_custom_size {
            self.generation_width = preset_w;
            self.generation_height = preset_h;
            self.generation_unlock_dimensions = false;
        } else {
            if custom_changed {
                self.generation_width = preset_w;
                self.generation_height = preset_h;
            }

            let unlock_changed = ui
                .checkbox(
                    &mut self.generation_unlock_dimensions,
                    "Unlock Width / Height",
                )
                .changed();

            let (_, rw, rh) =
                GENERATION_SIZES[self.generation_size.min(GENERATION_SIZES.len() - 1)];

            let snap = |n: f32| resolution::snap(n,self.generation_dimension_step);
            if previous_step!=self.generation_dimension_step {
                self.generation_width=snap(self.generation_width as f32);
                self.generation_height=snap(self.generation_height as f32);
            }

            if previous_ratio != self.generation_size {
                let long_side = self.generation_width.max(self.generation_height);
                self.generation_width = snap(long_side as f32 * rw as f32 / rw.max(rh) as f32);
                self.generation_height = snap(long_side as f32 * rh as f32 / rw.max(rh) as f32);
            }

            if unlock_changed && !self.generation_unlock_dimensions {
                self.generation_height = snap(self.generation_width as f32 * rh as f32 / rw as f32);
            }

            if self.generation_unlock_dimensions {
                crate::interaction::label(ui,"Width and height can be changed independently.");
            } else {
                crate::interaction::label(ui,"Aspect ratio locked. Editing either dimension adjusts the other.");
            }

            ui.horizontal(|ui| {
                crate::interaction::label(ui,"Width");
                let width_changed = ui
                    .add(
                        numeric::DragValue::new(&mut self.generation_width)
                            .range(256..=4096)
                            .speed(self.generation_dimension_step),
                    )
                    .changed();

                crate::interaction::label(ui,"Height");
                let height_changed = ui
                    .add(
                        numeric::DragValue::new(&mut self.generation_height)
                            .range(256..=4096)
                            .speed(self.generation_dimension_step),
                    )
                    .changed();

                self.generation_width = snap(self.generation_width as f32);
                self.generation_height = snap(self.generation_height as f32);

                if !self.generation_unlock_dimensions {
                    if width_changed {
                        let max_width = (4096.0_f32 * rw as f32 / rh as f32).min(4096.0);
                        self.generation_width = snap((self.generation_width as f32).min(max_width));
                        self.generation_height =
                            snap(self.generation_width as f32 * rh as f32 / rw as f32);
                    } else if height_changed {
                        let max_height = (4096.0_f32 * rh as f32 / rw as f32).min(4096.0);
                        self.generation_height =
                            snap((self.generation_height as f32).min(max_height));
                        self.generation_width =
                            snap(self.generation_height as f32 * rw as f32 / rh as f32);
                    }
                }
            });
        }

        crate::interaction::label(ui,format!(
            "Output: {} × {} ({}-pixel increments)",
            self.generation_width, self.generation_height, self.generation_dimension_step
        ));
        let vram = self.dependency_manager.diagnostics.as_ref().and_then(compatibility::vram_gib);
        let guidance = resolution::recommend(&self.generation_checkpoint, vram, self.generation_width, self.generation_height);
        crate::interaction::small(ui,&guidance.explanation);
        if guidance.caution {ui.colored_label(egui::Color32::YELLOW,"Higher memory risk: try the suggested size or a smaller model if generation fails.");}
        if ui.small_button("Apply suggested resolution").clicked() {
            self.generation_custom_size = true;
            self.generation_width = guidance.width;
            self.generation_height = guidance.height;
        }
        ui.horizontal(|ui| {
            crate::interaction::label(ui,"Batch");
            for count in [1, 2, 4, 8] {
                ui.selectable_value(&mut self.generation_batch_count, count, count.to_string());
            }
        });
        ui.checkbox(&mut self.generation_random_seeds, "Random seeds per image");
        crate::interaction::label(ui,"When disabled, seeds increment from the starting seed.");
        crate::interaction::label(ui,"Steps");
        ui.add(numeric::Slider::new(&mut self.generation_steps, 1..=100));
        if ai_generation::ignores_negative(&self.generation_checkpoint) {crate::interaction::label(ui,"This model uses CFG 1");} else {
            crate::interaction::label(ui,"CFG");
            ui.add(numeric::Slider::new(&mut self.generation_cfg, 1.0..=20.0));
        }
        if ui.small_button("Reset to recommended").clicked() {
            self.reset_generation_sampling();
        }
        ui.horizontal(|ui| {
            crate::interaction::label(ui,"Seed");
            ui.add(numeric::DragValue::new(&mut self.generation_seed));
        });
    }

    fn save_generation_project(&mut self, save_as:bool) {
        self.capture_workspace_editor();
        let Some(mut path)=(!save_as).then(||self.generation_project_path.clone()).flatten().or_else(||rfd::FileDialog::new().add_filter("PixelForge Project",&["pxf"]).set_file_name("Generation.pxf").save_file()) else {return};
        path.set_extension("pxf");
        let current=GenerationSettings {prompt:self.generation_prompt.clone(),negative:self.generation_negative.clone(),auto_negative:self.generation_auto_negative,checkpoint:self.generation_checkpoint.clone(),style:self.generation_style,ratio:self.generation_size,custom_size:self.generation_custom_size,unlock_dimensions:self.generation_unlock_dimensions,resolution:self.generation_resolution,dimension_step:self.generation_dimension_step,width:self.generation_width,height:self.generation_height,steps:self.generation_steps,cfg:self.generation_cfg,seed:self.generation_seed,refine_parent:None,refine_strength:None,refine_prompt:None,refine_masked:false};
        let mut current_json = settings_json(&current);
        current_json["edit_checkpoint"] = serde_json::json!(self.edit_checkpoint);
        current_json["qwen_steps"] = serde_json::json!(self.qwen_steps);
        current_json["flux_steps"] = serde_json::json!(self.flux_steps);
        current_json["thumbnail_groups"] =
            serde_json::json!(self.thumbnail_groups.iter()
            .map(|group| serde_json::json!({"name": group.name, "collapsed": group.collapsed}))
            .collect::<Vec<_>>());
        let project = generation_project::GalleryProject {
            items: self
                .generation_gallery
                .iter()
                .map(|i| generation_project::GalleryImage {
                    settings: {
                        let mut value = settings_json(&i.settings);
                        value["editor"] = i.editor.to_json();
                        value["thumbnail_group"] = serde_json::json!(i.group);
                        value
                    },
                    pixels: i.pixels.clone(),
                    seed: i.seed,
                })
                .collect(),
            selected: self.generation_selected,
            compare_index: self.generation_compare_index,
            compare: self.generation_compare,
            current: current_json,
        };
        match generation_project::save(&path, &project) {
            Ok(()) => {
                self.generation_project_path = Some(path.clone());
                self.project_dirty = false;
                self.generation_status = format!("Saved {}", path.display());
            }
            Err(e) => self.generation_status = format!("Save failed: {e}"),
        }
    }
    fn open_generation_project(&mut self,path:&std::path::Path,ctx:&egui::Context) {
        let project=match generation_project::load(path) {Ok(p)=>p,Err(e)=>{self.generation_status=format!("Open failed: {e}");return}};
        for item in &project.items {
            if let Err(error)=studio::EditorState::from_json(&item.settings["editor"],item.pixels.width(),item.pixels.height()) {self.generation_status=format!("Open failed: {error}");return;}
        }
        self.create_project_tab();
        self.blank_project = false;
        self.workspace_editor = None;
        self.thumbnail_groups = project.current["thumbnail_groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|group| ThumbnailGroup {
                name: group["name"]
                    .as_str()
                    .unwrap_or("Group")
                    .chars()
                    .take(100)
                    .collect(),
                collapsed: group["collapsed"].as_bool().unwrap_or(false),
            })
            .collect();
        let gallery: Vec<GeneratedItem> = project
            .items
            .into_iter()
            .enumerate()
            .map(|(n, i)| {
                let editor = studio::EditorState::from_json(
                    &i.settings["editor"],
                    i.pixels.width(),
                    i.pixels.height(),
                )
                .expect("validated Studio state");
                let display = editor.render(&i.pixels);
                let size = [i.pixels.width() as usize, i.pixels.height() as usize];
                let texture = ctx.load_texture(
                    format!("loaded_generation_{n}"),
                    ColorImage::from_rgba_unmultiplied(size, display.as_raw()),
                    TextureOptions::LINEAR,
                );
                let group = i.settings["thumbnail_group"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|&index| index < self.thumbnail_groups.len());
                GeneratedItem {
                    group,
                    editor,
                    settings: settings_from_json(&i.settings),
                    pixels: i.pixels,
                    texture,
                    seed: i.seed,
                }
            })
            .collect();
        self.edit_checkpoint = project.current["edit_checkpoint"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        self.qwen_steps = project.current["qwen_steps"]
            .as_u64()
            .filter(|steps| (1..=100).contains(steps))
            .unwrap_or(20) as u32;
        self.flux_steps = project.current["flux_steps"]
            .as_u64()
            .filter(|steps| (1..=100).contains(steps))
            .unwrap_or(4) as u32;
        let current = settings_from_json(&project.current);
        self.generation_prompt = current.prompt;
        self.generation_negative = current.negative;
        self.generation_auto_negative = current.auto_negative;
        self.generation_checkpoint = current.checkpoint;
        self.generation_style = current.style;
        self.generation_size = current.ratio;
        self.generation_custom_size = current.custom_size;
        self.generation_unlock_dimensions = current.unlock_dimensions;
        self.generation_resolution = current.resolution;
        self.generation_dimension_step = current.dimension_step;
        self.generation_width = current.width;
        self.generation_height = current.height;
        self.generation_steps = current.steps;
        self.generation_cfg = current.cfg;
        self.generation_seed = current.seed;
        self.select_default_checkpoint();
        self.refinement_source = None;
        self.refinement_mask.clear();
        self.refinement_mask_texture=None;
        self.conversation_draft.clear();
        self.generation_gallery=gallery;self.generation_selected=project.selected.min(self.generation_gallery.len().saturating_sub(1));self.generation_compare_index=project.compare_index.min(self.generation_gallery.len().saturating_sub(1));self.generation_compare=project.compare && self.generation_gallery.len()>1;self.generation_project_path=Some(path.to_path_buf());self.generation_workspace=true;self.generation_status=format!("Opened {} ({} images)",path.display(),self.generation_gallery.len());
    }
    fn import_generation_images(&mut self,ctx:&egui::Context) {
        if let Some(paths)=rfd::FileDialog::new().add_filter("Images",&["png","jpg","jpeg","webp"]).pick_files() {
            let mut count=0;
            for path in paths {match image::open(&path) {
                Ok(image)=>{let pixels=image.to_rgba8();let size=[pixels.width() as usize,pixels.height() as usize];let texture=ctx.load_texture(format!("import_{}_{}",self.generation_gallery.len(),count),ColorImage::from_rgba_unmultiplied(size,pixels.as_raw()),TextureOptions::LINEAR);let settings=GenerationSettings {prompt:String::new(),negative:String::new(),auto_negative:false,checkpoint:"Imported image".into(),style:0,ratio:0,custom_size:true,unlock_dimensions:true,resolution:0,dimension_step:32,width:pixels.width(),height:pixels.height(),steps:0,cfg:0.0,seed:0,refine_parent:None,refine_strength:None,refine_prompt:None,refine_masked:false};self.blank_project=false;self.generation_gallery.push(GeneratedItem {group:None, editor: studio::EditorState::default(),settings,pixels,texture,seed:0});self.generation_selected=self.generation_gallery.len()-1;count+=1;},
                Err(e)=>self.generation_status=format!("Import failed: {e}"),
            }}
            if count>0 {self.project_dirty=true;self.generation_status=format!("Imported {count} image(s)");}
        }
    }
    fn export_generation_selected(&mut self,format:image::ImageFormat,ext:&str) {
        self.capture_workspace_editor();
        let Some(item)=self.generation_gallery.get(self.generation_selected) else {return};
        let filename=format!("PixelForge_{}_{}.{}",self.generation_selected+1,item.seed,ext);
        if let Some(path)=rfd::FileDialog::new().add_filter(ext,&[ext]).set_file_name(&filename).save_file() {
            self.generation_status=match item.editor.render(&item.pixels).save_with_format(&path,format) {Ok(())=>format!("Exported {}",path.display()),Err(e)=>format!("Export failed: {e}")};
        }
    }

    fn submit_conversation(&mut self, ctx: &egui::Context, edit: bool) {
        let draft = if edit { &self.conversation_draft } else { &self.generation_prompt };
        let model = editing_checkpoint(&self.generation_checkpoint, &self.edit_checkpoint, edit);
        if self.generation_running || draft.trim().is_empty() || model.is_empty() { return; }
        let initial_prompt = self.generation_prompt.clone();
        if edit {
            let Some(source) = self.generation_gallery.get(self.generation_selected) else { return; };
            let painted = self.refinement_masked;
            if painted && !self.has_edit_selection() {
                self.generation_status = "Select an area before applying an edit to selection".into();
                return;
            }
            if painted {self.refinement_mask=self.selection_mask.as_ref().map(|m|m.pixels().to_vec()).unwrap_or_default();}
            self.refinement_source = Some(self.generation_selected);
            self.generation_prompt = conversation_context(&source.settings);
            self.refinement_prompt = self.conversation_draft.trim().to_owned();
            self.generation_width = ((source.pixels.width().clamp(256,4096)+32)/64*64).clamp(256,4096);
            self.generation_height = ((source.pixels.height().clamp(256,4096)+32)/64*64).clamp(256,4096);
            self.generation_custom_size = true;
            self.generation_unlock_dimensions = true;
            self.refinement_masked = painted;
        } else {
            self.refinement_source = None;
            self.refinement_masked = false;
        }
        self.start_generation(ctx);
        self.generation_prompt = initial_prompt;
    }

    fn show_generation_progress(&mut self, ui: &mut egui::Ui) {
        if self.generation_running {
            let step = self.generation_progress.load(std::sync::atomic::Ordering::Relaxed);
            let settings = self.generation_batch_settings.as_ref();
            let steps = settings.map(|s| s.steps).unwrap_or(self.generation_steps).max(1);
            let total = if settings.and_then(|s| s.refine_parent).is_some() { 1 } else { self.generation_batch_count.max(1) };
            let fraction = ((self.generation_batch_done as f32 + (step as f32 / steps as f32).clamp(0.0,1.0)) / total as f32).clamp(0.0,1.0);
            ui.add(egui::ProgressBar::new(fraction).animate(true).text(format!("Image {} / {} · Step {} / {} · {:.0}%", (self.generation_batch_done+1).min(total), total, step, steps, fraction*100.0)));
            ui.horizontal_wrapped(|ui| {
                if step == 0 { ui.spinner(); crate::interaction::label(ui,"Preparing / waiting for ComfyUI…"); }
                if let Some(started) = self.generation_started { crate::interaction::label(ui,format!("{:.1}s elapsed", started.elapsed().as_secs_f32())); }
                if let Some(settings) = settings { crate::interaction::label(ui,format!("Model: {}", settings.checkpoint)); }
                if ui.button("Cancel").clicked() { self.generation_cancel.store(true, std::sync::atomic::Ordering::Relaxed); }
            });
        }
        let status = if self.generation_status.is_empty() {
            &self.generation_models_status
        } else {
            &self.generation_status
        };
        crate::interaction::small(ui, status);
    }

    fn show_conversation(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Initial prompt");
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::CollapsingHeader::new(egui::RichText::new("Generation Settings").heading().strong())
                .show(ui, |ui| self.show_generation_settings(ui));
            crate::interaction::small(ui, "Style, size, steps, and CFG");
        });
        theme::section(ui, "Select Model");
        let checkpoint_before_selection = self.generation_checkpoint.clone();
        let model_row_height = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Button)) + ui.spacing().item_spacing.y;
        let generation_menu_height = model_row_height * (self.generation_checkpoints.len() + 2) as f32 + 16.0;
        egui::ComboBox::from_id_salt("initial_generation_model").width(ui.available_width()).height(generation_menu_height).truncate()
            .selected_text(if self.generation_checkpoint.is_empty() {
                "Select a checkpoint"
            } else {
                &self.generation_checkpoint
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.generation_checkpoint,ai_generation::FLUX_KLEIN.into(),format!("{} — Fast generation, prompt-driven images, instruction-based editing",ai_generation::FLUX_KLEIN));
                ui.selectable_value(&mut self.generation_checkpoint,ai_generation::Z_IMAGE.into(),format!("{} — Fast photographic generation, natural lighting",ai_generation::Z_IMAGE));
                for checkpoint in &self.generation_checkpoints {
                    let lower = checkpoint.to_ascii_lowercase();
                    let best_for = if lower.contains("cyberrealistic") || lower.contains("pony") {
                        "Realistic portraits, character images, detailed photography"
                    } else if lower.contains("realvis") {
                        "Natural photography, portraits, landscapes"
                    } else if lower.contains("juggernaut") {
                        "Detailed images, cinematic scenes, dramatic lighting"
                    } else if lower.contains("sd_xl_base") || lower.contains("sdxl_base") {
                        "General-purpose images, illustration, prompt testing"
                    } else {
                        "Custom checkpoint"
                    };
                    ui.selectable_value(
                        &mut self.generation_checkpoint,
                        checkpoint.clone(),
                        format!("{checkpoint} — {best_for}"),
                    );
                }
            });
        if self.generation_checkpoint != checkpoint_before_selection {
            let (steps, cfg, _) = Self::recommended_generation_settings(&self.generation_checkpoint);
            self.reset_generation_sampling();
            self.generation_status = format!("Applied recommended settings: {steps} steps, CFG {cfg:.1}. You can still adjust them manually.");
        }
        self.prompt_spelling.edit(ui,&mut self.generation_prompt,0,"Describe a new image…",4);
        let width = ui.available_width();
        let button_height = ui.spacing().interact_size.y.max(
            ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y,
        );
        let can_generate = !self.generation_running && !self.ai_running
            && !self.generation_prompt.trim().is_empty() && !self.generation_checkpoint.is_empty();
        let create = egui::RichText::new("Create New Image").size(ui.text_style_height(&egui::TextStyle::Button) * 1.25);
        let create_clicked = ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_enabled(can_generate, theme::primary_text(create).min_size(egui::vec2(width, button_height * 2.0))).clicked()
        }).inner;
        if create_clicked {
            self.submit_conversation(ui.ctx(), false);
        }
        ui.horizontal(|ui| {
            let size = egui::vec2(((width - ui.spacing().item_spacing.x) * 0.5).max(0.0), button_height);
            if ui.add_enabled(!self.generation_prompt.is_empty(), egui::Button::new("Clear Prompt").min_size(size)).clicked() {
                self.generation_prompt.clear();
            }
            if ui.add(egui::Button::new("Prompt Guide").min_size(size)).clicked() {
                self.prompt_guide.open = !self.prompt_guide.open;
            }
        });
        if self.prompt_guide.open {
            self.prompt_guide.show(
                ui.ctx(),
                &self.generation_checkpoint,
                &mut self.generation_prompt,
                &mut self.prompt_spelling,
            );
        }
        ui.separator();
        theme::section(ui, "Negative prompt");
        if self.generation_checkpoint==ai_generation::Z_IMAGE {
            crate::interaction::label(ui,"Z-Image Turbo uses CFG 1: negative prompts do not affect its output.");
        }
        if self.generation_checkpoint==ai_generation::FLUX_KLEIN || editing_checkpoint(&self.generation_checkpoint,&self.edit_checkpoint,true)==ai_generation::FLUX_KLEIN {
            crate::interaction::label(ui,"FLUX Klein uses CFG 1: negative prompts do not affect its output.");
        }
        ui.checkbox(&mut self.generation_auto_negative,"Use automatic negative prompts")
            .on_hover_text("Adds the selected style's exclusions to SDXL generation and editing. Qwen uses your typed negative prompt only.");
        if self.generation_auto_negative {
            let automatic=automatic_negative_prompt(GENERATION_STYLES[self.generation_style.min(GENERATION_STYLES.len()-1)].0);
            crate::interaction::label(ui,format!("Automatic (SDXL): {automatic}"));
        }
        self.prompt_spelling.edit(ui,&mut self.generation_negative,2,"Additional negative prompts…",4);
        ui.separator();
        theme::section(ui, "Changes prompt");
        let edit_menu_height = model_row_height * (self.generation_checkpoints.len() + 4) as f32 + 16.0;
        egui::ComboBox::from_id_salt("edit_model").width(ui.available_width()).height(edit_menu_height).truncate().selected_text(if self.edit_checkpoint.is_empty() { "Use generation model" } else { &self.edit_checkpoint }).show_ui(ui, |ui| {
            ui.selectable_value(&mut self.edit_checkpoint, String::new(), "Use generation model");
            for name in [ai_generation::QWEN_EDIT,ai_generation::QWEN_EDIT_2511,ai_generation::FLUX_KLEIN] {
                ui.selectable_value(&mut self.edit_checkpoint,name.into(),name);
            }
            for model in &self.generation_checkpoints { ui.selectable_value(&mut self.edit_checkpoint, model.clone(), model); }
        });
        crate::interaction::label(ui,format!("Edit model: {}", editing_checkpoint(&self.generation_checkpoint, &self.edit_checkpoint, true)));
        let edit_model=editing_checkpoint(&self.generation_checkpoint,&self.edit_checkpoint,true).to_owned();
        if edit_model==ai_generation::Z_IMAGE {
            crate::interaction::label(ui,"Z-Image Turbo creates new images. Choose FLUX, Qwen, or an SDXL model above to apply edits.");
        }
        if ai_generation::is_qwen(&edit_model) {
            crate::interaction::small(ui,"Instruction-based editing · CFG 4");
            crate::interaction::label(ui,"Qwen steps");
            ui.spacing_mut().slider_width=(ui.available_width()-80.0).max(60.0);
            ui.add(numeric::Slider::new(&mut self.qwen_steps,1..=100))
                .on_hover_text("2509 starts at 20 steps; 2511 recommends 40. Fewer steps reduce sampling time but may reduce quality; model loading and encoding still take time.");
        } else if edit_model==ai_generation::FLUX_KLEIN {
            crate::interaction::small(ui,"FLUX instruction editing · CFG 1 · 4 steps recommended");
            crate::interaction::label(ui,"FLUX edit steps");
            ui.spacing_mut().slider_width=(ui.available_width()-80.0).max(60.0);
            ui.add(numeric::Slider::new(&mut self.flux_steps,1..=100));
        } else {
            crate::interaction::small(ui,"SDXL image-to-image refinement. Choose Qwen Edit for instruction-based editing.");
        }
        self.prompt_spelling.edit(ui,&mut self.conversation_draft,1,"Describe changes to the viewed image…",4);
        let has_image = self.generation_gallery.get(self.generation_selected).is_some();
        if has_image { crate::interaction::label(ui,format!("Editing version {} · earlier versions are kept", self.generation_selected+1)); }
        else { crate::interaction::label(ui,"Create or import an image before applying changes."); }
        if !ai_generation::is_instruction_model(&edit_model) {
            crate::interaction::label(ui,"Change strength");
            ui.spacing_mut().slider_width=(ui.available_width()-80.0).max(60.0);
            ui.add(numeric::Slider::new(&mut self.refinement_strength, 0.01..=1.0));
        }
        let ready = has_image && edit_model!=ai_generation::Z_IMAGE && !self.generation_running && !self.ai_running && !self.conversation_draft.trim().is_empty() && !editing_checkpoint(&self.generation_checkpoint, &self.edit_checkpoint, true).is_empty();
        let has_selection = self.has_edit_selection();
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(ready, theme::primary("Apply Edit")).on_hover_text("Edit the whole image").clicked() {
                self.refinement_masked=false;
                self.submit_conversation(ui.ctx(), true);
            }
            if ui.add_enabled(ready && has_selection, egui::Button::new("Apply Edit to Selection"))
                .on_hover_text("Change the selected area and preserve everything outside it. Qwen edits the whole image internally, then only the selected pixels are kept.").clicked() {
                self.refinement_masked=true;
                self.submit_conversation(ui.ctx(), true);
            }
        });
        if self.generation_checkpoints.is_empty() && ui.button("Connect / Refresh Models").clicked() { self.refresh_generation_models(ui.ctx()); }
    }

    fn show_generation_workspace(&mut self, ui: &mut egui::Ui) {
        if self.blank_project {
            crate::interaction::label(ui, "Import an image, or choose Generate to create one.");
            self.show_workspace_filmstrip(ui);
            return;
        }
        self.bind_workspace_editor(ui.ctx());
        if self.generation_gallery.is_empty() {
            crate::interaction::strong(ui, "Image Studio");
            crate::interaction::label(ui, "Import an image, or choose Generate to create one.");
            self.show_workspace_filmstrip(ui);
            return;
        }
        let count = self.generation_gallery.len();
        self.generation_selected = self.generation_selected.min(count-1);
        self.generation_compare_index = self.generation_compare_index.min(count-1);
        ui.horizontal_wrapped(|ui| {
            crate::interaction::strong(ui,"Image Studio");
            ui.separator();
            crate::interaction::strong(ui,self.generation_gallery[self.generation_selected].version_label(self.generation_selected));
            crate::interaction::small(ui,format!("{} × {}", self.image_size[0],self.image_size[1]));
            egui::ComboBox::from_id_salt("compare_left_version")
                .selected_text(format!("Left: {}", self.generation_gallery[self.generation_compare_index].version_label(self.generation_compare_index)))
                .show_ui(ui, |ui| {
                    for index in 0..count { ui.selectable_value(&mut self.generation_compare_index, index, self.generation_gallery[index].version_label(index)); }
                });
            ui.add_enabled(count > 1 && self.crop.is_none(), egui::Checkbox::new(&mut self.generation_compare, "Compare"));
            egui::ComboBox::from_id_salt("compare_right_version")
                .selected_text(format!("Right: {}", self.generation_gallery[self.generation_selected].version_label(self.generation_selected)))
                .show_ui(ui, |ui| {
                    for index in 0..count { ui.selectable_value(&mut self.generation_selected, index, self.generation_gallery[index].version_label(index)); }
                });
            if self.has_edit_selection() {
                ui.colored_label(egui::Color32::from_rgb(255,150,150),"Selection active");
                ui.checkbox(&mut self.selection_show_mask,"Show selection");
            }
            if ui.button("Select Area").clicked() {
                self.crop=None;
                self.generation_compare=false;
                self.selection_tools_tab=true;self.ai_tools_tab=false;self.generation_tools_tab=false;
                self.selection_brush_enabled=true;
            }
            if ui.button("Fit").clicked() {
                if let Some(crop)=&mut self.crop {crop.reset_view();}
                self.generation_zoom = 1.0;
                self.generation_pan = Vec2::ZERO;
            }
            if ui.button("−").clicked() {
                if let Some(crop)=&mut self.crop {crop.zoom_by(1.0/1.25);} else {self.generation_zoom = (self.generation_zoom / 1.25).max(0.1);}
            }
            if ui.button("+").clicked() {
                if let Some(crop)=&mut self.crop {crop.zoom_by(1.25);} else {self.generation_zoom = (self.generation_zoom * 1.25).min(16.0);}
            }
            let can_rotate = !self.generation_running && !self.ai_running && !self.ai_saving && !self.exporting && self.crop.is_none();
            if ui.add_enabled(can_rotate, egui::Button::new("Rotate Left")).on_hover_text("Rotate this version 90° counterclockwise. Later edits use this orientation.").clicked() {
                self.rotate_workspace_version(ui.ctx(), false);
            }
            if ui.add_enabled(can_rotate, egui::Button::new("Rotate Right")).on_hover_text("Rotate this version 90° clockwise. Later edits use this orientation.").clicked() {
                self.rotate_workspace_version(ui.ctx(), true);
            }
            if ui.add_enabled(!self.generation_running && !self.ai_running && self.crop.is_none(),egui::Button::new("+ Save New Version"))
                .on_hover_text("Save the current edits as a new version. Use File > Save Project to save the whole history to disk.").clicked() {
                self.keep_workspace_version(ui.ctx(),"Manual edits");
            }
            if ui.add_enabled(!self.generation_running && !self.ai_running && self.crop.is_none(),egui::Button::new("Remove Version")).clicked() {
                self.remove_workspace_version(self.generation_selected);
            }

        });
        if self.generation_gallery.is_empty() { return; }
        self.bind_workspace_editor(ui.ctx());
        let available = ui.available_size();
        if self.crop.is_some() {
            let size = egui::vec2(
                available.x,
                (available.y - self.workspace_filmstrip_height(ui)).max(1.0),
            );
            if let Some(texture) = self.processed_texture.as_ref().or(self.texture.as_ref()) {
                let texture = texture.id();
                let dimensions = [self.image_size[0] as u32, self.image_size[1] as u32];
                ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                    self.crop.as_mut().unwrap().show(ui, texture, dimensions);
                });
            }
            self.show_workspace_filmstrip(ui);
            return;
        }
        if !self.generation_compare {
            // The shared viewer also supports legacy comparison modes. The Studio
            // checkbox is authoritative, including after loading an image/project.
            self.comparison_enabled = false;
            self.version_comparison = false;
            self.divider_dragging = false;
            let size = egui::vec2(
                available.x,
                (available.y - self.workspace_filmstrip_height(ui)).max(1.0),
            );
            let fit = (size.x / self.image_size[0].max(1) as f32)
                .min(size.y / self.image_size[1].max(1) as f32);
            self.zoom = fit * self.generation_zoom;
            self.offset = self.generation_pan;
            self.fit_pending = false;
            self.selection_brush_enabled =
                self.selection_tools_tab && !self.generation_running && !self.ai_running;
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                self.show_viewer(ui)
            });
            self.generation_zoom = self.zoom / fit.max(0.000001);
            self.generation_pan = self.offset;
            self.show_workspace_filmstrip(ui);
            return;
        }
        // Reserve only the filmstrip; image uses all remaining canvas space.
        let viewer_height = (available.y - self.workspace_filmstrip_height(ui)).max(1.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available.x.max(100.0), viewer_height),
            egui::Sense::click_and_drag(),
        );
        // Both versions share the canvas and camera; clipping reveals each side.
        let divider_x = rect.left() + rect.width() * self.generation_compare_position;
        let divider = egui::Rect::from_min_max(
            egui::pos2(divider_x - 9.0, rect.top()),
            egui::pos2(divider_x + 9.0, rect.bottom()),
        );
        let divider_response = ui.interact(divider, ui.id().with("studio_compare_divider"), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if response.double_clicked() || divider_response.double_clicked() {
            self.generation_zoom = 1.0;
            self.generation_pan = Vec2::ZERO;
        } else if divider_response.dragged() {
            if let Some(pointer) = divider_response.interact_pointer_pos() {
                self.generation_compare_position = ((pointer.x-rect.left())/rect.width()).clamp(0.0,1.0);
            }
        } else if response.dragged() {
            self.generation_pan += response.drag_delta();
        }
        if response.hovered() || divider_response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let previous = self.generation_zoom;
                self.generation_zoom = (previous * (scroll * 0.002).exp()).clamp(0.1,16.0);
                if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                    let relative = pointer-rect.center();
                    self.generation_pan = relative-(relative-self.generation_pan)*(self.generation_zoom/previous);
                }
            }
        }
        self.capture_workspace_editor();
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));
        let divider_x = rect.left() + rect.width()*self.generation_compare_position;
        for (slot,index) in [self.generation_compare_index,self.generation_selected].into_iter().enumerate() {
            let item = &self.generation_gallery[index];
            let size = egui::vec2(item.pixels.width() as f32,item.pixels.height() as f32);
            let fit = (rect.width()/size.x).min(rect.height()/size.y);
            let image_rect = egui::Rect::from_center_size(rect.center()+self.generation_pan,size*fit*self.generation_zoom);
            let clip = if slot==0 {
                egui::Rect::from_min_max(rect.min,egui::pos2(divider_x,rect.bottom()))
            } else {
                egui::Rect::from_min_max(egui::pos2(divider_x,rect.top()),rect.max)
            };
            painter.with_clip_rect(clip).image(item.texture.id(),image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),egui::Color32::WHITE);
        }
        if self.selection_show_mask && self.has_edit_selection() {
            if let Some(selection)=&self.selection_texture {
                let item=&self.generation_gallery[self.generation_selected];
                let size=egui::vec2(item.pixels.width() as f32,item.pixels.height() as f32);
                let fit=(rect.width()/size.x).min(rect.height()/size.y);
                let image_rect=egui::Rect::from_center_size(rect.center()+self.generation_pan,size*fit*self.generation_zoom);
                let clip=egui::Rect::from_min_max(egui::pos2(divider_x,rect.top()),rect.max);
                painter.with_clip_rect(clip).image(selection.id(),image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),egui::Color32::WHITE);
            }
        }
        painter.line_segment([egui::pos2(divider_x,rect.top()),egui::pos2(divider_x,rect.bottom())],egui::Stroke::new(2.0,egui::Color32::WHITE));
        painter.circle_filled(egui::pos2(divider_x,rect.center().y),12.0,egui::Color32::from_gray(45));
        painter.text(egui::pos2(divider_x,rect.center().y),egui::Align2::CENTER_CENTER,"↔",egui::FontId::proportional(16.0),egui::Color32::WHITE);

        self.show_workspace_filmstrip(ui);
    }

    fn has_edit_selection(&self) -> bool {
        self.selection_mask.as_ref().is_some_and(|mask| {
            mask.pixels().len()==self.image_size[0]*self.image_size[1] && mask.pixels().iter().any(|&v|v>0)
        })
    }

    fn remove_workspace_version(&mut self, removed:usize) {
        if self.generation_running || self.ai_running || removed>=self.generation_gallery.len() {return;}
        self.rename_version=None;
        self.capture_workspace_editor();
        self.workspace_editor=None;
        self.original_pixels=None;
        self.generation_gallery.remove(removed);
        let remap=|index:Option<usize>|match index {Some(i) if i==removed=>None,Some(i) if i>removed=>Some(i-1),other=>other};
        if self.refinement_source==Some(removed) {self.refinement_mask.clear();self.refinement_mask_texture=None;}
        self.refinement_source=remap(self.refinement_source);
        self.workspace_ai_parent=remap(self.workspace_ai_parent);
        for item in &mut self.generation_gallery {item.settings.refine_parent=remap(item.settings.refine_parent);}
        let last=self.generation_gallery.len().saturating_sub(1);
        self.generation_selected=remap(Some(self.generation_selected)).unwrap_or(removed).min(last);
        self.generation_compare_index=remap(Some(self.generation_compare_index)).unwrap_or(removed).min(last);
        self.generation_compare &= self.generation_gallery.len()>1;
        if self.generation_gallery.is_empty() {
            self.texture=None;self.processed_texture=None;self.preview_source=None;self.image_size=[0,0];
            self.selection_mask=None;
        }
        self.project_dirty=true;
        self.generation_status="Removed image version (original files were not deleted)".into();
    }

    fn workspace_filmstrip_height(&self, ui: &egui::Ui) -> f32 {
        // Fixed thumbnail cells and captions, with only the scrollbar gutter below.
        let group_header = if self.thumbnail_groups.is_empty() {
            0.0
        } else {
            ui.spacing().interact_size.y + ui.spacing().item_spacing.y
        };
        group_header
            + ui.spacing().interact_size.y
            + ui.spacing().item_spacing.y
            + 96.0
            + ui.text_style_height(&egui::TextStyle::Small)
            + ui.spacing().item_spacing.y * 2.0
            + ui.spacing().scroll.bar_width
            + 8.0
    }

    fn show_workspace_details(&self,ui:&mut egui::Ui) {
        let Some(item)=self.generation_gallery.get(self.generation_selected) else {return};
        let details=format!("Version {} · {} × {} · Model: {} · Seed: {} · Steps: {} · CFG: {}",
            self.generation_selected+1,item.pixels.width(),item.pixels.height(),item.settings.checkpoint,item.seed,item.settings.steps,item.settings.cfg);
        crate::interaction::label_widget(ui,egui::Label::new(&details).truncate()).on_hover_text(&details);
        let prompts=format!("Prompt: {}{} · Negative: {}",item.settings.prompt,
            item.settings.refine_prompt.as_ref().map(|p|format!(" · Changes: {p}")).unwrap_or_default(),item.settings.negative);
        crate::interaction::label_widget(ui,egui::Label::new(&prompts).truncate()).on_hover_text(&prompts);
    }

    fn move_thumbnail(&mut self, drop: ThumbnailDrop) {
        let ThumbnailDrop {
            index,
            group,
            before,
        } = drop;
        let len = self.generation_gallery.len();
        if self.generation_running || self.ai_running || index >= len || before > len {
            return;
        }
        let destination = before - usize::from(index < before);
        if index == destination && self.generation_gallery[index].group == group {
            return;
        }
        let mut item = self.generation_gallery.remove(index);
        item.group = group;
        self.generation_gallery.insert(destination, item);
        let remap = |old| {
            if old == index {
                destination
            } else {
                let shifted = old - usize::from(old > index);
                shifted + usize::from(shifted >= destination)
            }
        };
        self.generation_selected = remap(self.generation_selected);
        self.generation_compare_index = remap(self.generation_compare_index);
        self.workspace_editor = self.workspace_editor.map(remap);
        self.workspace_ai_parent = self.workspace_ai_parent.map(remap);
        self.refinement_source = self.refinement_source.map(remap);
        if let Some((index, _)) = &mut self.rename_version {
            *index = remap(*index);
        }
        for item in &mut self.generation_gallery {
            item.settings.refine_parent = item
                .settings
                .refine_parent
                .filter(|&parent| parent < len)
                .map(remap);
        }
        self.project_dirty = true;
    }

    fn delete_thumbnail_group(&mut self, group: usize, keep_images: bool) {
        if self.generation_running || self.ai_running || group >= self.thumbnail_groups.len() {
            return;
        }
        if !keep_images {
            for index in (0..self.generation_gallery.len()).rev() {
                if self.generation_gallery[index].group == Some(group) {
                    self.remove_workspace_version(index);
                }
            }
        }
        self.thumbnail_groups.remove(group);
        for item in &mut self.generation_gallery {
            item.group = match item.group {
                Some(index) if index == group => None,
                Some(index) if index > group => Some(index - 1),
                other => other,
            };
        }
        self.project_dirty = true;
    }

    fn open_thumbnails_as_project(
        &mut self,
        ctx: &egui::Context,
        indices: &[usize],
        group: Option<usize>,
    ) {
        if indices.is_empty() {
            return;
        }
        self.capture_workspace_editor();
        let named_group = group
            .and_then(|index| self.thumbnail_groups.get(index))
            .cloned();
        let items = indices
            .iter()
            .map(|&index| {
                let mut item = self.generation_gallery[index].clone();
                item.settings.refine_parent = item
                    .settings
                    .refine_parent
                    .and_then(|parent| indices.iter().position(|&source| source == parent));
                item.group = named_group.as_ref().map(|_| 0);
                let preview = item.editor.render(&item.pixels);
                item.texture = ctx.load_texture(
                    format!("copied_thumbnail_{index}"),
                    ColorImage::from_rgba_unmultiplied(
                        [preview.width() as usize, preview.height() as usize],
                        preview.as_raw(),
                    ),
                    TextureOptions::LINEAR,
                );
                item
            })
            .collect();
        self.create_blank_project_tab();
        self.generation_gallery = items;
        if let Some(mut group) = named_group {
            group.collapsed = false;
            self.thumbnail_groups.push(group);
        }
        self.blank_project = false;
        self.generation_workspace = true;
        self.project_dirty = true;
        self.bind_workspace_editor(ctx);
    }

    fn show_group_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.group_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut done = false;
        match &mut dialog {
            GroupDialog::Name { group, name } => {
                let title = if group.is_some() {
                    "Rename Thumbnail Group"
                } else {
                    "New Thumbnail Group"
                };
                egui::Window::new(title)
                    .id(egui::Id::new(("thumbnail_group_name", self.active_tab)))
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        let edit = interaction::singleline(ui, name);
                        let clean: String = name.trim().chars().take(100).collect();
                        ui.horizontal(|ui| {
                            let save = ui
                                .add_enabled(!clean.is_empty(), egui::Button::new("Save Group"))
                                .clicked()
                                || (!clean.is_empty()
                                    && edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                            if save {
                                if let Some(index) = *group {
                                    self.thumbnail_groups[index].name = clean;
                                } else {
                                    self.thumbnail_groups.push(ThumbnailGroup {
                                        name: clean,
                                        collapsed: false,
                                    });
                                }
                                self.project_dirty = true;
                                done = true;
                            }
                            done |= ui.button("Cancel").clicked();
                        });
                    });
            }
            GroupDialog::Delete(group) => {
                egui::Window::new("Delete Thumbnail Group")
                    .id(egui::Id::new(("thumbnail_group_delete", self.active_tab)))
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label(format!(
                            "Delete the group ‘{}’?",
                            self.thumbnail_groups[*group].name
                        ));
                        ui.horizontal(|ui| {
                            let idle = !self.generation_running && !self.ai_running;
                            if ui
                                .add_enabled(idle, egui::Button::new("Keep Images"))
                                .clicked()
                            {
                                self.delete_thumbnail_group(*group, true);
                                done = true;
                            }
                            if ui
                                .add_enabled(idle, egui::Button::new("Remove Images"))
                                .clicked()
                            {
                                self.delete_thumbnail_group(*group, false);
                                done = true;
                            }
                            done |= ui.button("Cancel").clicked();
                        });
                    });
            }
        }
        if open && !done {
            self.group_dialog = Some(dialog);
        }
    }

    fn show_workspace_filmstrip(&mut self, ui: &mut egui::Ui) {
        self.capture_workspace_editor();
        ui.separator();
        if ui.small_button("+ New Group").clicked() {
            self.group_dialog = Some(GroupDialog::Name {
                group: None,
                name: String::new(),
            });
        }
        let mut remove_version = None;
        let mut rename_version = None;
        let mut open_project = None;
        let mut moved = None;
        let busy = self.generation_running || self.ai_running;
        let project = self.active_tab;
        let drop_target = |response: &egui::Response, group, before, after| {
            if busy {
                return None;
            }
            let drag = response.dnd_hover_payload::<ThumbnailDrag>()?;
            if drag.project != project {
                return None;
            }
            let pointer = response.ctx.input(|input| input.pointer.hover_pos())?;
            let insert_after = pointer.x >= response.rect.center().x;
            response.ctx.layer_painter(response.layer_id).rect_stroke(
                response.interact_rect,
                3.0,
                egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
                egui::StrokeKind::Inside,
            );
            response
                .dnd_release_payload::<ThumbnailDrag>()
                .map(|_| ThumbnailDrop {
                    index: drag.index,
                    group,
                    before: if insert_after { after } else { before },
                })
        };

        ui.spacing_mut().scroll.floating = false;
        ui.style_mut().always_scroll_the_only_direction = true;
        egui::ScrollArea::horizontal()
            .id_salt(("studio_filmstrip", project))
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                ..Default::default()
            })
            .max_height(self.workspace_filmstrip_height(ui))
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let grouped = !self.thumbnail_groups.is_empty();
                    for group in
                        std::iter::once(None).chain((0..self.thumbnail_groups.len()).map(Some))
                    {
                        let indices: Vec<_> = self
                            .generation_gallery
                            .iter()
                            .enumerate()
                            .filter_map(|(index, item)| (item.group == group).then_some(index))
                            .collect();
                        ui.vertical(|ui| {
                            if grouped {
                                let label = group.map_or_else(
                                    || "Ungrouped".to_owned(),
                                    |index| {
                                        let group = &self.thumbnail_groups[index];
                                        format!(
                                            "{} {} ({})",
                                            if group.collapsed { "▸" } else { "▾" },
                                            group.name,
                                            indices.len()
                                        )
                                    },
                                );
                                let header = ui.button(label).on_hover_text(
                                    "Drop a thumbnail here to move it to this section",
                                );
                                let end = indices
                                    .last()
                                    .map_or(self.generation_gallery.len(), |index| index + 1);
                                moved = drop_target(&header, group, end, end).or(moved);
                                if let Some(index) = group {
                                    if header.clicked() {
                                        self.thumbnail_groups[index].collapsed =
                                            !self.thumbnail_groups[index].collapsed;
                                        self.project_dirty = true;
                                    }
                                    header.context_menu(|ui| {
                                        if ui.button("Rename Group…").clicked() {
                                            self.group_dialog = Some(GroupDialog::Name {
                                                group,
                                                name: self.thumbnail_groups[index].name.clone(),
                                            });
                                            ui.close();
                                        }
                                        if ui
                                            .add_enabled(!busy, egui::Button::new("Delete Group…"))
                                            .clicked()
                                        {
                                            self.group_dialog = Some(GroupDialog::Delete(index));
                                            ui.close();
                                        }
                                        ui.add_enabled_ui(!indices.is_empty(), |ui| {
                                            ui.menu_button("Open Group as New Project", |ui| {
                                                if ui
                                                    .button("Open as Individual Thumbnails")
                                                    .clicked()
                                                {
                                                    open_project = Some((indices.clone(), None));
                                                    ui.close();
                                                }
                                                if ui.button("Keep Named Group").clicked() {
                                                    open_project = Some((indices.clone(), group));
                                                    ui.close();
                                                }
                                            });
                                        });
                                    });
                                    if self.thumbnail_groups[index].collapsed {
                                        return;
                                    }
                                }
                            }

                            ui.horizontal(|ui| {
                                for &index in &indices {
                                    let item = &self.generation_gallery[index];
                                    ui.vertical(|ui| {
                                        let size = item.texture.size_vec2();
                                        let thumb = size * (92.0 / size.x.max(size.y));
                                        let sense = if busy {
                                            egui::Sense::click()
                                        } else {
                                            egui::Sense::click_and_drag()
                                        };
                                        let response = ui
                                            .add_sized(
                                                [116.0, 96.0],
                                                egui::Button::image(egui::Image::new((
                                                    item.texture.id(),
                                                    thumb,
                                                )))
                                                .sense(sense)
                                                .selected(index == self.generation_selected),
                                            )
                                            .on_hover_text(
                                                item.settings
                                                    .refine_prompt
                                                    .as_deref()
                                                    .unwrap_or(&item.settings.prompt),
                                            );
                                        if !busy {
                                            response.dnd_set_drag_payload(ThumbnailDrag {
                                                project,
                                                index,
                                            });
                                            moved = drop_target(&response, group, index, index + 1)
                                                .or(moved);
                                        }
                                        if response.clicked() {
                                            self.generation_selected = index;
                                            self.refinement_source = None;
                                            self.refinement_mask_texture = None;
                                        }
                                        response.context_menu(|ui| {
                                            if ui.button("Open as New Project").clicked() {
                                                open_project = Some((vec![index], None));
                                                ui.close();
                                            }
                                            if ui.button("Rename Version…").clicked() {
                                                rename_version =
                                                    Some((index, item.editor.name.clone()));
                                                ui.close();
                                            }
                                            if ui
                                                .add_enabled(
                                                    !busy,
                                                    egui::Button::new("Remove Image Version"),
                                                )
                                                .clicked()
                                            {
                                                remove_version = Some(index);
                                                ui.close();
                                            }
                                        });
                                        let caption = item.version_label(index);
                                        ui.add_sized(
                                            [116.0, ui.text_style_height(&egui::TextStyle::Small)],
                                            egui::Label::new(egui::RichText::new(&caption).small())
                                                .truncate(),
                                        )
                                        .on_hover_text(caption);
                                    });
                                }
                            });
                        });
                        if grouped {
                            ui.separator();
                        }
                    }
                });
            });

        if let Some(drop) = moved {
            self.move_thumbnail(drop);
        }
        if let Some((indices, group)) = open_project {
            self.open_thumbnails_as_project(ui.ctx(), &indices, group);
            return;
        }
        self.show_group_dialog(ui.ctx());

        if let Some(index) = remove_version {
            self.remove_workspace_version(index);
        }
        if let Some(rename) = rename_version {
            self.rename_version = Some(rename);
        }
        if let Some((index, mut name)) = self.rename_version.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Rename Version")
                .id(egui::Id::new(("rename_thumbnail", self.active_tab)))
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    crate::interaction::label(
                        ui,
                        "Version name (leave empty to use the version number)",
                    );
                    let response = interaction::singleline(ui, &mut name);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
                    }
                    ui.horizontal(|ui| {
                        save |= ui.button("Save Name").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                    cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
                });
            if save {
                if let Some(item) = self.generation_gallery.get_mut(index) {
                    item.editor.name = name.trim().chars().take(100).collect();
                    self.project_dirty = true;
                }
            } else if open && !cancel {
                self.rename_version = Some((index, name));
            }
        }
    }

    // A tab owns its editor, gallery and worker receivers. Only application services
    // and window state stay with the active shell when the tab changes.
    fn swap_app_state(&mut self, other: &mut PixelForge) {
        std::mem::swap(&mut self.generation_checkpoints, &mut other.generation_checkpoints);
        std::mem::swap(&mut self.generation_models_rx, &mut other.generation_models_rx);
        std::mem::swap(&mut self.generation_models_status, &mut other.generation_models_status);
        std::mem::swap(&mut self.dependency_manager, &mut other.dependency_manager);
        std::mem::swap(&mut self.controls, &mut other.controls);
        std::mem::swap(&mut self.show_settings, &mut other.show_settings);
        std::mem::swap(&mut self.display, &mut other.display);
        std::mem::swap(&mut self.exit_allowed, &mut other.exit_allowed);
        std::mem::swap(&mut self.exit_confirmation, &mut other.exit_confirmation);
        std::mem::swap(&mut self.file_drops, &mut other.file_drops);
        std::mem::swap(&mut self.install_cancel, &mut other.install_cancel);
        std::mem::swap(&mut self.install_folder, &mut other.install_folder);
        std::mem::swap(
            &mut self.install_folder_manual,
            &mut other.install_folder_manual,
        );
        std::mem::swap(
            &mut self.install_model_index,
            &mut other.install_model_index,
        );
        std::mem::swap(&mut self.install_progress, &mut other.install_progress);
        std::mem::swap(&mut self.install_rx, &mut other.install_rx);
        std::mem::swap(&mut self.install_status, &mut other.install_status);
        std::mem::swap(&mut self.install_total, &mut other.install_total);
        std::mem::swap(&mut self.interaction, &mut other.interaction);
        std::mem::swap(&mut self.model_library_open, &mut other.model_library_open);
        std::mem::swap(&mut self.prompt_guide, &mut other.prompt_guide);
        std::mem::swap(&mut self.remove_model, &mut other.remove_model);
        std::mem::swap(&mut self.updater, &mut other.updater);
    }

    fn poll_project_jobs(&mut self, ctx: &egui::Context) {
        self.poll_export(ctx);
        self.poll_preview(ctx);
        self.poll_ai_upscale(ctx);
        self.poll_generation(ctx);
        self.poll_ai_save(ctx);
    }

    fn poll_open_projects(&mut self, ctx: &egui::Context) {
        self.poll_model_catalog(ctx);
        self.poll_project_jobs(ctx);
        for (_, project) in &mut self.inactive_tabs {
            project.poll_project_jobs(ctx);
        }
    }

    fn backend_jobs_running(&self) -> bool {
        self.generation_running || self.ai_running || self.generation_models_rx.is_some()
    }

    fn cancel_project_jobs(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        self.generation_cancel.store(true, Relaxed);
        self.ai_cancel.store(true, Relaxed);
    }

    fn project_tab_title(&self) -> String {
        self.generation_project_path
            .as_ref()
            .or(self.project_path.as_ref())
            .or(self.image_path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    fn switch_project_tab(&mut self, target: usize) {
        self.capture_workspace_editor();
        if target == self.active_tab {
            return;
        }
        let mut tabs = std::mem::take(&mut self.inactive_tabs);
        let Some(position) = tabs.iter().position(|(id, _)| *id == target) else {
            self.inactive_tabs = tabs;
            return;
        };
        let previous = self.active_tab;
        let next_id = self.next_tab;
        let closing = self.closing_tab;
        std::mem::swap(self, tabs[position].1.as_mut());
        self.swap_app_state(tabs[position].1.as_mut());
        tabs[position].0 = previous;
        self.inactive_tabs = tabs;
        self.active_tab = target;
        self.select_default_checkpoint();
        self.next_tab = next_id;
        self.closing_tab = closing;
    }

    fn create_project_tab(&mut self) {
        self.capture_workspace_editor();
        if self.original_pixels.is_none()
            && self.generation_gallery.is_empty()
            && self.generation_project_path.is_none()
            && !self.project_dirty
            && !self.generation_running
            && !self.ai_running
            && !self.exporting
            && !self.ai_saving
        {
            return;
        }
        self.create_blank_project_tab();
        self.blank_project = false;
    }

    fn create_blank_project_tab(&mut self) {
        self.capture_workspace_editor();
        let mut tabs = std::mem::take(&mut self.inactive_tabs);
        let previous = self.active_tab;
        let id = self.next_tab;
        let mut old = std::mem::replace(self, PixelForge::default());
        self.swap_app_state(&mut old);
        self.select_default_checkpoint();
        tabs.push((previous, Box::new(old)));
        self.inactive_tabs = tabs;
        self.active_tab = id;
        self.next_tab = id + 1;
        self.blank_project = true;
    }

    fn close_project_tab(&mut self, target: usize) {
        if target == self.active_tab {
            if let Some((next, _)) = self.inactive_tabs.last() {
                let next = *next;
                self.switch_project_tab(next);
                self.close_project_tab(target);
            } else {
                self.close_image();
            }
        } else {
            if let Some(position) = self.inactive_tabs.iter().position(|(id, _)| *id == target) {
                self.inactive_tabs.remove(position).1.cancel_project_jobs();
            }
        }
        self.closing_tab = None;
    }

    fn show_project_tabs(&mut self, ctx: &egui::Context) {
        let mut entries: Vec<(usize, String, bool)> = self
            .inactive_tabs
            .iter()
            .map(|(id, tab)| (*id, tab.project_tab_title(), tab.project_dirty))
            .collect();
        entries.push((
            self.active_tab,
            self.project_tab_title(),
            self.project_dirty,
        ));
        entries.sort_by_key(|entry| entry.0);
        let mut switch = None;
        let mut close = None;
        egui::TopBottomPanel::top("document_tabs").show(ctx, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (id, title, dirty) in &entries {
                        let label = format!("{}{}", title, if *dirty { " *" } else { "" });
                        if ui.selectable_label(*id == self.active_tab, label).clicked() {
                            switch = Some(*id);
                        }
                        if ui
                            .small_button("×")
                            .on_hover_text("Close project tab")
                            .clicked()
                        {
                            close = Some(*id);
                        }
                        ui.separator();
                    }
                    ui.scope(|ui| {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
                        let response = response.on_hover_text("New or open project");
                        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "New or open project"));
                        let visuals = ui.style().interact(&response);
                        ui.painter().rect(rect, 4.0, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
                        let center = rect.center();
                        let stroke = egui::Stroke::new(1.2_f32, visuals.fg_stroke.color);
                        ui.painter().line_segment([center-egui::vec2(4.0,0.0), center+egui::vec2(4.0,0.0)], stroke);
                        ui.painter().line_segment([center-egui::vec2(0.0,4.0), center+egui::vec2(0.0,4.0)], stroke);
                        egui::Popup::menu(&response).show(|ui| {
                            if ui.button("New Blank Project").clicked() {
                                self.create_blank_project_tab();
                                ui.close();
                            }
                            if ui.button("Open Image…").clicked() {
                                ui.close();
                                self.open_image(ctx);
                            }
                            if ui.button("Open Project…").clicked() {
                                ui.close();
                                self.open_project(ctx);
                            }
                        });
                    });
                });
            });
        });
        if let Some(id) = switch {
            self.switch_project_tab(id);
        }
        if let Some(id) = close {
            let dirty = if id == self.active_tab {
                self.project_dirty
            } else {
                self.inactive_tabs
                    .iter()
                    .find(|(tab_id, _)| *tab_id == id)
                    .is_some_and(|(_, tab)| tab.project_dirty)
            };
            if dirty {
                self.closing_tab = Some(id);
            } else {
                self.close_project_tab(id);
            }
        }
        if let Some(id) = self.closing_tab {
            egui::Window::new("Unsaved Project Changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    crate::interaction::label(ui,"Save this project's changes before closing its tab?");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.closing_tab = None;
                        }
                        if ui.button("Discard").clicked() {
                            self.close_project_tab(id);
                        }
                        if ui.button("Save").clicked() {
                            let current = self.active_tab;
                            self.switch_project_tab(id);
                            if self.generation_workspace {self.save_generation_project(false);} else {self.save_project(false);}
                            if !self.project_dirty {
                                self.close_project_tab(id);
                            } else if current != id {
                                self.switch_project_tab(current);
                            }
                        }
                    });
                });
        }
    }

    fn request_open(&mut self, action: PendingOpen, ctx: &egui::Context) {
        if action != PendingOpen::Close {
            self.perform_open(action, ctx);
        } else if self.project_dirty {
            self.pending_open = Some(action);
        } else {
            self.perform_open(action, ctx);
        }
    }

    fn perform_open(&mut self, action: PendingOpen, ctx: &egui::Context) {
        match action {
            PendingOpen::Image => self.open_image(ctx),
            PendingOpen::Project => self.open_project(ctx),
            PendingOpen::Close => {
                if self.inactive_tabs.is_empty() {
                    self.close_image();
                } else {
                    self.close_project_tab(self.active_tab);
                }
            }
        }
    }

    fn close_image(&mut self) {
        self.cancel_project_jobs();
        let id = self.next_tab;
        let mut previous = std::mem::take(self);
        self.swap_app_state(&mut previous);
        self.select_default_checkpoint();
        self.active_tab = id;
        self.next_tab = id + 1;
        self.blank_project = true;
    }

    fn show_exit_confirmation(&mut self, ctx: &egui::Context) {
        if !self.exit_confirmation {
            return;
        }

        egui::Window::new("Save changes before exiting?")
            .collapsible(false)
            .resizable(false)
            .auto_sized()
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let messages = [
                    "Your project has unsaved changes.",
                    "Save your work to keep the image versions and edits.",
                ];
                // Measure at the current UI scale; center the natural-size buttons
                // as a group rather than stretching them or right-aligning the row.
                let text_width = |text: &str, style: egui::TextStyle| {
                    ui.painter().layout_no_wrap(text.into(), style.resolve(ui.style()), ui.visuals().text_color()).size().x
                };
                let button_width: f32 = ["Cancel", "Exit Without Saving", "Save & Exit"].iter()
                    .map(|label| (text_width(label, egui::TextStyle::Button) + ui.spacing().button_padding.x * 2.0).max(ui.spacing().interact_size.x))
                    .sum::<f32>() + ui.spacing().item_spacing.x * 2.0;
                let content_width = messages.iter().map(|message| text_width(message, egui::TextStyle::Body))
                    .fold(button_width, f32::max);
                ui.set_width(content_width);
                ui.vertical_centered(|ui| {
                    for message in messages {crate::interaction::label(ui,message);}
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - button_width) * 0.5).max(0.0));
                    if ui.button("Cancel").clicked() || ui.input(|i|i.key_pressed(egui::Key::Escape)) {
                        self.exit_confirmation = false;
                    }
                    if ui.button("Exit Without Saving").clicked() {
                        self.exit_confirmation = false;
                        self.exit_allowed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.add(theme::primary("Save & Exit")).clicked() {
                        let mut dirty: Vec<_> = self
                            .inactive_tabs
                            .iter()
                            .filter_map(|(id, tab)| tab.project_dirty.then_some(*id))
                            .collect();
                        if self.project_dirty {
                            dirty.insert(0, self.active_tab);
                        }
                        let mut saved = true;
                        for id in dirty {
                            self.switch_project_tab(id);
                            if self.generation_workspace {
                                self.save_generation_project(false);
                            } else {
                                self.save_project(false);
                            }
                            if self.project_dirty {
                                saved = false;
                                break;
                            }
                        }
                        if saved {
                            self.exit_confirmation = false;
                            self.exit_allowed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
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
            .id(egui::Id::new("unsaved-before-replacement"))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                crate::interaction::label(ui,"This image has unsaved project changes.");
                match action {
                    PendingOpen::Close => {
                        crate::interaction::label(ui,"Closing this image will discard those changes.");
                    }
                    _ => {
                        crate::interaction::label(ui,"Opening another file will discard those changes.");
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
        // Exit confirmation must use the same project and existing path as File > Save.
        if self.generation_workspace {
            self.save_generation_project(save_as);
            return;
        }
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
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            vibrance: self.vibrance,
            gamma: self.gamma,
            comparison_enabled: self.comparison_enabled,
            comparison_position: self.comparison_position,
            selection_adjustments: [
                self.selection_adjustments.brightness,
                self.selection_adjustments.contrast,
                self.selection_adjustments.exposure,
                self.selection_adjustments.saturation,
                self.selection_adjustments.temperature,
                self.selection_adjustments.tint,
                self.selection_adjustments.highlights,
                self.selection_adjustments.shadows,
                self.selection_adjustments.whites,
                self.selection_adjustments.blacks,
                self.selection_adjustments.vibrance,
                self.selection_adjustments.gamma,
            ],
            active_mask: self
                .selection_mask
                .as_ref()
                .map(|mask| mask.pixels().to_vec()),
            selection_target: self.selection_adjustment_target,
            active_raster_version: self.active_raster_version,
            edit_versions: self.edit_versions.iter().map(|version| project::ProjectVersion {
                global: adjustment_values(version.global),
                local: adjustment_values(version.local),
                mask: version.mask.as_ref().map(|mask| mask.pixels().to_vec()),
                raster: version.raster.clone(),
            }).collect(),
            saved_selections: self
                .saved_selections
                .iter()
                .map(|(name, mask)| (name.clone(), mask.pixels().to_vec()))
                .collect(),
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
        let Some(path) = rfd::FileDialog::new().add_filter("PixelForge Project", &["pxf"]).pick_file() else {return};
        self.open_project_path(ctx, path);
    }

    fn open_project_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        match generation_project::is_generation(&path) {
            Ok(true) => {self.open_generation_project(&path,ctx);return;}
            Ok(false) => {},
            Err(e) => {self.status=format!("Project open failed: {e}");return;}
        }
        self.open_editor_project_at(ctx, path);
    }

    fn open_editor_project_at(&mut self, ctx: &egui::Context, path: PathBuf) {
        // Load and validate before changing the current image.
        let data = match project::load(&path) {
            Ok(data) => data,
            Err(err) => {
                self.status = format!("Project open failed: {err}");
                return;
            }
        };

        self.create_project_tab();
        self.blank_project = false;
        let size = [data.pixels.width() as usize, data.pixels.height() as usize];

        let color = ColorImage::from_rgba_unmultiplied(size, data.pixels.as_raw());

        self.texture = Some(ctx.load_texture("current_image", color, TextureOptions::LINEAR));

        self.workspace_editor=None;
        self.generation_workspace=false;
        self.original_pixels = Some(data.pixels);
        self.edit_versions.clear();
        self.active_raster_version = None;
        self.prepare_preview_source();
        self.processed_texture = None;

        self.brightness = data.brightness;
        self.contrast = data.contrast;
        self.exposure = data.exposure;
        self.saturation = data.saturation;
        self.temperature = data.temperature;
        self.tint = data.tint;
        self.highlights = data.highlights;
        self.shadows = data.shadows;
        self.whites = data.whites;
        self.blacks = data.blacks;
        self.vibrance = data.vibrance;
        self.gamma = data.gamma;
        self.comparison_enabled = data.comparison_enabled;
        self.comparison_position = data.comparison_position;
        self.selection_adjustment_target = data.selection_target;
        self.selection_adjustments = adjustments::Adjustments {
            brightness: data.selection_adjustments[0],
            contrast: data.selection_adjustments[1],
            exposure: data.selection_adjustments[2],
            saturation: data.selection_adjustments[3],
            temperature: data.selection_adjustments[4],
            tint: data.selection_adjustments[5],
            highlights: data.selection_adjustments[6],
            shadows: data.selection_adjustments[7],
            whites: data.selection_adjustments[8],
            blacks: data.selection_adjustments[9],
            vibrance: data.selection_adjustments[10],
            gamma: data.selection_adjustments[11],
        };
        let restored_active_mask = data.active_mask;

        let selection_dimensions = self
            .original_pixels
            .as_ref()
            .map(|pixels| pixels.dimensions());

        self.edit_versions = data.edit_versions.into_iter().map(|version| {
            let mask = selection_dimensions.and_then(|(width, height)| {
                version.mask.and_then(|pixels| {
                    masking::SelectionMask::from_pixels(width, height, pixels).ok()
                })
            });
            EditVersion {
                global: adjustments_from_values(version.global),
                local: adjustments_from_values(version.local),
                mask,
                raster: version.raster,
            }
        }).collect();
        self.active_raster_version = data.active_raster_version;
        self.compare_version_a = 0;
        self.compare_version_b = self.edit_versions.len().saturating_sub(1);
        self.version_comparison = false;
        self.version_texture_a = None;
        self.version_texture_b = None;

        self.saved_selections = selection_dimensions
            .map(|(width, height)| {
                data.saved_selections
                    .into_iter()
                    .filter_map(|(name, pixels)| {
                        masking::SelectionMask::from_pixels(width, height, pixels)
                            .ok()
                            .map(|mask| (name, mask))
                    })
                    .collect()
            })
            .unwrap_or_default();

        self.selected_saved_selection = None;
        self.active_saved_selection = None;
        self.selection_name_input.clear();
        self.selection_mask = selection_dimensions.and_then(|(width, height)| {
            restored_active_mask
                .and_then(|pixels| masking::SelectionMask::from_pixels(width, height, pixels).ok())
        });
        self.selection_texture = None;
        self.selection_undo.clear();
        self.selection_redo.clear();

        self.selection_last_point = None;
        self.selection_stroke_active = false;
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

        self.activate_version_source(ctx);
        self.refresh_selection_overlay(ctx);

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

    fn can_edit_command(&self,ctx:&egui::Context,action:interaction::Action)->bool {
        use interaction::Action;
        if matches!(action,Action::Undo|Action::Redo) && !interaction::has_text_target(ctx) {
            return if action==Action::Undo {!self.selection_undo.is_empty()}else{!self.selection_redo.is_empty()};
        }
        interaction::enabled(ctx,action)
    }
    fn edit_command(&mut self,ctx:&egui::Context,action:interaction::Action) {
        use interaction::Action;
        if action==Action::Fullscreen {self.display.toggle();return;}
        if !self.can_edit_command(ctx,action) {return;}
        if !interaction::has_text_target(ctx) {
            match action {Action::Undo=>{self.selection_undo_action(ctx);return;},Action::Redo=>{self.selection_redo_action(ctx);return;},_=>{}}
        }
        interaction::queue(ctx,action);
    }
    fn show_controls_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let mut open = self.show_settings;
        let mut changed = false;

        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .default_width(450.0)
            .resizable(true)
            .vscroll(false)
            .show(ctx, |ui| {
                let tab_id = ui.id().with("settings_tab");
                let mut tab = ui.data(|d| d.get_temp::<usize>(tab_id)).unwrap_or(0);
                let previous = tab;
                ui.horizontal_wrapped(|ui| {
                    for (index, title) in ["Visuals", "Mouse Controls", "Hotkeys"].iter().enumerate() {
                        ui.selectable_value(&mut tab, index, *title);
                    }
                });
                if tab != previous { self.interaction.cancel_capture(); }
                ui.data_mut(|d| d.insert_temp(tab_id, tab));
                ui.separator();
                egui::ScrollArea::vertical().id_salt(("settings_content",tab)).max_height(580.0).show(ui, |ui| {
                if tab == 2 { self.interaction.hotkeys_ui(ui); }
                if tab == 0 {
                self.display.ui(ui);
                ui.add_space(12.0);
                ui.heading("Appearance");
                ui.separator();

                crate::interaction::label(ui,"User interface scale");

                let scale_changed = ui
                    .add(
                        numeric::Slider::new(&mut self.controls.ui_scale, 0.75..=2.0).percent()
                            .step_by(0.05)
                            .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
                    )
                    .changed();

                if scale_changed {
                    changed = true;
                    ctx.set_pixels_per_point(
                        ctx.native_pixels_per_point().unwrap_or(1.0) * self.controls.ui_scale,
                    );
                }

                if ui.button("Reset UI Scale to 100%").clicked() {
                    self.controls.ui_scale = 1.0;
                    changed = true;
                    ctx.set_pixels_per_point(ctx.native_pixels_per_point().unwrap_or(1.0));
                }

                crate::interaction::label(ui,"Changes apply immediately and are saved automatically.");

                ui.add_space(12.0);
                }
                if tab == 1 {
                ui.heading("Mouse Controls");
                ui.separator();

                crate::interaction::label(ui,"Pan image");

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
                crate::interaction::label(ui,"Fit image to window");

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
                crate::interaction::label(ui,"Mouse wheel zoom sensitivity");

                changed |= ui
                    .add(
                        numeric::Slider::new(&mut self.controls.zoom_sensitivity, 0.25..=3.0).percent()
                            .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
                    )
                    .changed();

                ui.add_space(10.0);
                ui.separator();

                if ui.button("Restore Default Controls").clicked() {
                    self.controls = Controls::default();
                    changed = true;
                }

                crate::interaction::label(ui,"Changes are saved automatically.");
                }
                });
            });

        self.show_settings = open;
        if !open {self.interaction.cancel_capture();}

        if changed {
            self.panning = false;
            self.save_controls();
        }
    }

    fn prepare_preview_source(&mut self) {
        use std::sync::atomic::Ordering;

        // Invalidate results from the previous image.
        self.preview_generation.fetch_add(1, Ordering::Relaxed);
        self.preview_rx = None;
        self.preview_source = None;
        self.processed_texture = None;

        let Some(original) = &self.original_pixels else {
            return;
        };

        // Edit previews use the exact source dimensions. The GPU already
        // displays the resulting egui texture; never downscale the edited
        // side relative to the original before/after comparison.
        self.histogram = histogram::Histogram::from_image(original);
        self.preview_source = Some(std::sync::Arc::new(original.clone()));
    }

    fn update_processed_image(&mut self, ctx: &egui::Context) {
        use std::sync::atomic::Ordering;

        let Some(original) = &self.preview_source else {
            return;
        };

        // Process at native resolution on the background worker. GPU texture
        // rendering remains hardware accelerated by egui/eframe.
        let source = std::sync::Arc::clone(original);
        let generation_counter = std::sync::Arc::clone(&self.preview_generation);

        let generation = generation_counter.fetch_add(1, Ordering::Relaxed) + 1;

        // Never show an old low-resolution adjusted texture stretched over
        // a full-resolution image while a new native-size render is pending.
        if self.processed_texture.as_ref().is_some_and(|texture| {
            texture.size() != [source.width() as usize, source.height() as usize]
        }) {
            self.processed_texture = None;
        }

        let settings = adjustments::Adjustments {
            brightness: self.brightness,
            contrast: self.contrast,
            exposure: self.exposure,
            saturation: self.saturation,
            temperature: self.temperature,
            tint: self.tint,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            vibrance: self.vibrance,
            gamma: self.gamma,
        };

        let selection_mask = self.selection_mask.clone();
        let local_settings = self.selection_adjustments;

        let (tx, rx) = std::sync::mpsc::channel();
        self.preview_rx = Some(rx);

        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let mut adjusted = (*source).clone();

            // Process one row at a time, checking for newer requests.
            let width = adjusted.width();
            let height = adjusted.height();
            for (y, row) in adjusted.rows_mut().enumerate() {
                if generation_counter.load(Ordering::Relaxed) != generation {
                    return;
                }
                for (x, pixel) in row.enumerate() {
                    settings.apply_pixel(pixel);
                    if let Some(mask) = &selection_mask {
                        let (mw, mh) = mask.dimensions();
                        if mw > 0 && mh > 0 {
                            let mx =
                                (x as u64 * mw as u64 / width as u64).min(mw as u64 - 1) as u32;
                            let my =
                                (y as u64 * mh as u64 / height as u64).min(mh as u64 - 1) as u32;
                            let alpha = mask.get(mx, my).unwrap_or(0);
                            if alpha > 0 {
                                let before = *pixel;
                                local_settings.apply_pixel(pixel);
                                for c in 0..3 {
                                    pixel[c] = ((before[c] as u32 * (255 - alpha) as u32
                                        + pixel[c] as u32 * alpha as u32
                                        + 127)
                                        / 255) as u8;
                                }
                            }
                        }
                    }
                }
            }

            if generation_counter.load(Ordering::Relaxed) != generation {
                return;
            }

            let size = [adjusted.width() as usize, adjusted.height() as usize];

            let hist = histogram::Histogram::from_image(&adjusted);
            let color = ColorImage::from_rgba_unmultiplied(size, adjusted.as_raw());

            if generation_counter.load(Ordering::Relaxed) != generation {
                return;
            }

            if tx.send((generation, color, hist)).is_ok() {
                ctx.request_repaint();
            }
        });
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        use std::sync::atomic::Ordering;

        let Some(rx) = &self.preview_rx else {
            return;
        };

        let result = rx.try_recv();

        match result {
            Ok((generation, color, hist)) => {
                if generation == self.preview_generation.load(Ordering::Relaxed) {
                    self.histogram = hist;
                    if let Some(texture) = &mut self.processed_texture {
                        texture.set(color, TextureOptions::LINEAR);
                    } else {
                        self.processed_texture = Some(ctx.load_texture(
                            "processed_image",
                            color,
                            TextureOptions::LINEAR,
                        ));
                    }
                }

                self.preview_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.preview_rx = None;
                self.status = "Adjusted preview worker stopped unexpectedly".into();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Keep the UI alive and make the pending render visible.
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
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
        let highlights = self.highlights;
        let shadows = self.shadows;
        let whites = self.whites;
        let blacks = self.blacks;
        let vibrance = self.vibrance;
        let gamma = self.gamma;
        let selection_mask = self.selection_mask.clone();
        let local_settings = self.selection_adjustments;

        let (tx, rx) = std::sync::mpsc::channel();

        self.export_rx = Some(rx);
        self.export_progress = 0.0;
        self.export_stage = "Starting export".into();
        self.export_encoding = false;
        self.exporting = true;
        self.export_result = None;
        self.last_export_path = None;
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
                    highlights,
                    shadows,
                    whites,
                    blacks,
                    vibrance,
                    gamma,
                };

                let total = adjusted.width() as usize * adjusted.height() as usize;

                let width = adjusted.width();
                let height = adjusted.height();
                for (index, pixel) in adjusted.pixels_mut().enumerate() {
                    settings.apply_pixel(pixel);
                    if let Some(mask) = &selection_mask {
                        let (mw, mh) = mask.dimensions();
                        if mw > 0 && mh > 0 {
                            let x = index as u64 % width as u64;
                            let y = index as u64 / width as u64;
                            let mx = (x * mw as u64 / width as u64).min(mw as u64 - 1) as u32;
                            let my = (y * mh as u64 / height as u64).min(mh as u64 - 1) as u32;
                            let alpha = mask.get(mx, my).unwrap_or(0);
                            if alpha > 0 {
                                let before = *pixel;
                                local_settings.apply_pixel(pixel);
                                for c in 0..3 {
                                    pixel[c] = ((before[c] as u32 * (255 - alpha) as u32
                                        + pixel[c] as u32 * alpha as u32
                                        + 127)
                                        / 255) as u8;
                                }
                            }
                        }
                    }
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
                            self.last_export_path = Some(path);
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
    fn refresh_vulkan_gpus(&mut self) {
        match ai_upscale::available_vulkan_gpus() {
            Ok(gpus) => {
                if !gpus.iter().any(|gpu| gpu.id == self.ai_gpu_id) {
                    self.ai_gpu_id = -1;
                }
                self.ai_gpus = gpus;
                self.ai_gpu_error = None;
            }
            Err(error) => {
                self.ai_gpu_id = -1;
                self.ai_gpus.clear();
                self.ai_gpu_error = Some(error);
            }
        }
    }

    fn submit_photo_restoration(&mut self, ctx: &egui::Context) {
        if self.generation_running || self.ai_running || self.ai_saving || self.exporting || self.dependency_manager.busy() || self.restoration_prompt.trim().is_empty() { return; }
        self.ensure_workspace(ctx);
        if self.generation_gallery.is_empty() { return; }
        // Reuse the normal local editing pipeline without overwriting Generate's controls.
        let draft = std::mem::replace(&mut self.conversation_draft, self.restoration_prompt.clone());
        let model = std::mem::replace(&mut self.edit_checkpoint, self.restoration_model.clone());
        let negative = std::mem::take(&mut self.generation_negative);
        let previous = (self.qwen_steps, self.flux_steps, self.refinement_masked,
            self.generation_width, self.generation_height, self.generation_custom_size, self.generation_unlock_dimensions);
        self.qwen_steps = self.restoration_steps;
        self.flux_steps = self.restoration_steps;
        self.refinement_masked = false;
        self.submit_conversation(ctx, true);
        self.conversation_draft = draft;
        self.edit_checkpoint = model;
        self.generation_negative = negative;
        (self.qwen_steps, self.flux_steps, self.refinement_masked,
            self.generation_width, self.generation_height, self.generation_custom_size, self.generation_unlock_dimensions) = previous;
    }

    fn show_photo_restoration(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::section(ui, "Photo Restoration");
        crate::interaction::label(ui,"Clean up the viewed photo. Each result becomes a new version at the original size.");
        let ready = self.original_pixels.is_some() && !self.ai_running && !self.ai_saving
            && !self.generation_running && !self.exporting && !self.dependency_manager.busy();
        theme::section(ui, "AI Restore");
        crate::interaction::label(ui,"Restore a grainy or damaged photo with FLUX or Qwen. Fine details may be reconstructed; compare faces with the original.");
        ui.add_enabled_ui(!self.generation_running && !self.ai_running, |ui| {
            crate::interaction::label(ui,"Restoration model");
            let previous = self.restoration_model.clone();
            egui::ComboBox::from_id_salt("photo_restoration_model").width(ui.available_width()).truncate()
                .selected_text(&self.restoration_model).show_ui(ui, |ui| {
                    for model in [ai_generation::FLUX_KLEIN, ai_generation::QWEN_EDIT, ai_generation::QWEN_EDIT_2511] {
                        ui.selectable_value(&mut self.restoration_model, model.into(), model);
                    }
                });
            if previous != self.restoration_model {
                self.restoration_steps = if self.restoration_model == ai_generation::FLUX_KLEIN {4} else if self.restoration_model == ai_generation::QWEN_EDIT {20} else {40};
            }
            crate::interaction::label(ui,"Restoration instructions");
            self.prompt_spelling.edit(ui, &mut self.restoration_prompt, 3, "Describe how to restore this photo…", 4);
            crate::interaction::label(ui,"Sampling steps");
            ui.spacing_mut().slider_width = (ui.available_width()-80.0).max(60.0);
            ui.add(numeric::Slider::new(&mut self.restoration_steps, 1..=100));
        });
        if ui.add_enabled(ready && !self.restoration_prompt.trim().is_empty(), theme::primary("Restore Photo")).clicked() {
            self.submit_photo_restoration(ctx);
        }
        crate::interaction::label(ui,"Uses the viewed version and keeps its size and orientation. The result is added to the right side of Compare.");
        ui.separator();
        crate::interaction::strong(ui,"Targeted cleanup");
        ui.add_enabled_ui(!self.ai_running, |ui| {
            crate::interaction::label(ui,"Cleanup preset");
            let before = self.restoration_preset;
            egui::ComboBox::from_id_salt("restoration_preset").width(ui.available_width()).truncate()
                .selected_text(["Gentle · preserve texture", "Grainy photo · reduce noise", "Soft photo · recover clarity"][self.restoration_preset.min(2)])
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.restoration_preset, 0, "Gentle · preserve texture");
                    ui.selectable_value(&mut self.restoration_preset, 1, "Grainy photo · reduce noise");
                    ui.selectable_value(&mut self.restoration_preset, 2, "Soft photo · recover clarity");
                });
            if before != self.restoration_preset {
                let (noise, strength) = restoration_preset_settings(self.restoration_preset);
                self.ai_denoise_level = noise;
                self.restoration_strength = strength;
            }
            crate::interaction::label(ui,"Strength for next cleanup");
            ui.spacing_mut().slider_width = (ui.available_width()-80.0).max(60.0);
            ui.add(numeric::Slider::new(&mut self.restoration_strength, 0.0..=1.0).custom_formatter(|v,_|format!("{:.0}%",v*100.0)));
            crate::interaction::label(ui,"Noise reduction");
            egui::ComboBox::from_id_salt("restoration_noise").width(ui.available_width()).truncate()
                .selected_text(["Very light", "Light", "Medium", "Strong"][self.ai_denoise_level.min(3) as usize])
                .show_ui(ui, |ui| {
                    for (level, name) in ["Very light", "Light", "Medium", "Strong"].iter().enumerate() {
                        ui.selectable_value(&mut self.ai_denoise_level, level as u32, *name);
                    }
                });
        });
        crate::interaction::label(ui,"1. Reduce grain and speckles");
        if ui.add_enabled(ready, theme::primary("Denoise Photo")).on_hover_text("Waifu2x noise-only model; keeps the original dimensions.").clicked() {
            self.start_ai_job(ctx, 1, true);
        }
        crate::interaction::label(ui,"2. Improve clarity if needed");
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(ready, egui::Button::new("Reduce Blur")).on_hover_text("NAFNet deblurring. Best for mild photographic blur; processing can take time.").clicked() {
                self.start_restoration_job(ctx, 1, false, Some(ai_restoration::Operation::Deblur));
            }
            if ui.add_enabled(ready, egui::Button::new("Sharpen Details")).on_hover_text("Real-ESRGAN assists luminance detail while preserving the original colors.").clicked() {
                self.start_restoration_job(ctx, 1, false, Some(ai_restoration::Operation::Sharpen));
            }
        });
        crate::interaction::label(
            ui,
            "Try one step, then compare with the original using Compare above the image. Enlarge last using Upscaling below.",
        );
        if ui.button("Open Dependencies").clicked() {
            self.dependency_manager.open = true;
        }
        ui.separator();
    }

    fn start_ai_upscale(&mut self, ctx: &egui::Context, scale: u32) {
        self.start_ai_job(ctx, scale, false);
    }

    fn start_ai_job(&mut self, ctx: &egui::Context, scale: u32, denoise: bool) {
        self.start_restoration_job(ctx, scale, denoise, None);
    }

    fn start_restoration_job(
        &mut self,
        ctx: &egui::Context,
        scale: u32,
        denoise: bool,
        restoration: Option<ai_restoration::Operation>,
    ) {
        if self.ai_running || self.ai_saving || self.exporting || self.generation_running || self.dependency_manager.busy() {
            self.ai_status = "Another operation is running".into();
            return;
        }

        self.bind_workspace_editor(ctx);
        self.capture_workspace_editor();
        let Some(original) = &self.original_pixels else {
            self.ai_status = "Open an image first".into();
            return;
        };

        if !denoise && restoration.is_none() {
            if let Err(error) =
                ai_upscale::validate_upscale_size(original.width(), original.height(), scale)
            {
                self.ai_status = error;
                return;
            }
        }

        let settings = adjustments::Adjustments {
            brightness: self.brightness,
            contrast: self.contrast,
            exposure: self.exposure,
            saturation: self.saturation,
            temperature: self.temperature,
            tint: self.tint,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            vibrance: self.vibrance,
            gamma: self.gamma,
        };

        let pixels = self.workspace_editor.and_then(|i|self.generation_gallery.get(i)).map(|item|item.editor.render(&item.pixels)).unwrap_or_else(||original.clone());
        let settings = if self.workspace_editor.is_some() {adjustments::Adjustments::default()} else {settings};
        let options = ai_upscale::UpscaleOptions {
            model: ["realesrgan-x4plus", "realesrgan-x4plus-anime"][self.ai_model.min(1)].into(),
            gpu_id: if self.ai_gpu_id == -1 {
                ai_upscale::preferred_gpu_id(&self.ai_gpus).unwrap_or(-1)
            } else {
                self.ai_gpu_id
            },
            tile_size: self.ai_tile_size,
        };
        let (tx, rx) = std::sync::mpsc::channel();

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Keep the user's result blend across operations; cleanup has a separate strength.
        self.ai_cancel = cancel.clone();
        self.ai_rx = Some(rx);
        self.workspace_ai_parent=self.workspace_editor;
        self.ai_running = true;
        self.ai_is_denoise = denoise;
        self.ai_restoration = restoration;
        self.ai_status = if let Some(operation) = restoration {
            format!("Running AI {}...", operation.name())
        } else if denoise {
            "Running AI photo denoise...".into()
        } else {
            format!("Running Real-ESRGAN {scale}x on GPU...")
        };

        self.ai_detail_rx = None;
        self.ai_detail_key = None;
        self.ai_detail_rect = None;
        self.ai_detail_ai_texture = None;
        self.ai_detail_lanczos_texture = None;
        self.ai_detail_status.clear();

        // Each inference receives its own cache, preventing older
        // background workers from populating the new result's cache.
        self.ai_decode_cache = std::sync::Arc::new(std::sync::Mutex::new(None));
        self.ai_result = None;
        self.ai_result_texture = None;
        self.ai_baseline_texture = None;
        self.ai_inspect_texture = None;
        self.ai_inspect_size = [0, 0];
        self.ai_inspect_zoom = 1.0;
        self.ai_inspect_pan = Vec2::ZERO;
        self.ai_inspect_fit = true;
        self.ai_result_path = None;
        self.ai_show_result = false;

        let ctx = ctx.clone();

        let denoise_level = self.ai_denoise_level;
        std::thread::spawn(move || {
            let result = if let Some(operation) = restoration {
                match operation {
                    ai_restoration::Operation::Sharpen => ai_restoration::sharpen(
                        pixels,
                        settings,
                        options.gpu_id,
                        options.tile_size,
                        cancel,
                    ),
                    ai_restoration::Operation::Deblur => {
                        ai_restoration::run(pixels, settings, operation, cancel)
                    }
                }
            } else if denoise {
                ai_denoise::denoise(
                    pixels,
                    settings,
                    denoise_level,
                    options.gpu_id,
                    options.tile_size,
                    cancel,
                )
            } else {
                ai_upscale::upscale(pixels, settings, scale, options, cancel)
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn poll_ai_upscale(&mut self, ctx: &egui::Context) {
        if !self.ai_running {
            return;
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(150));

        let result = self.ai_rx.as_ref().and_then(|rx| rx.try_recv().ok());

        let Some(result) = result else {
            return;
        };

        self.ai_running = false;
        self.ai_rx = None;
        self.ai_cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);

        match result {
            Ok(result) => {
                self.ai_result_size = result.dimensions;

                let baseline_size = [
                    result.baseline_preview.width() as usize,
                    result.baseline_preview.height() as usize,
                ];

                let baseline_color = ColorImage::from_rgba_unmultiplied(
                    baseline_size,
                    result.baseline_preview.as_raw(),
                );

                self.ai_baseline_texture = Some(ctx.load_texture(
                    "ai_lanczos_comparison",
                    baseline_color,
                    TextureOptions::LINEAR,
                ));

                self.ai_inspect_size = [
                    result.preview.width() as usize,
                    result.preview.height() as usize,
                ];

                let operation = if let Some(kind) = self.ai_restoration {
                    kind.name()
                } else if self.ai_is_denoise {
                    "AI denoise"
                } else {
                    "AI upscale"
                };
                self.ai_status = format!(
                    "{operation} complete: {} x {}.",
                    result.dimensions[0], result.dimensions[1]
                );

                self.status = self.ai_status.clone();
                let saved = std::sync::Arc::new(result);
                self.ai_variations.push((
                    format!("{} #{}", operation, self.ai_variations.len() + 1),
                    saved.clone(),
                ));
                self.ai_compare_b = self.ai_variations.len() - 1;
                self.ai_compare_a = self.ai_compare_b.saturating_sub(1);
                self.ai_result = Some(saved);
                self.refresh_ai_preview(ctx);
                self.ai_show_result = true;
                if self.generation_workspace {
                    match self.append_ai_workspace_result(ctx,operation) {
                        Ok(()) => self.ai_status.push_str(" Added to version history; save the project to keep it."),
                        Err(error) => self.ai_status=error,
                    }
                }
                self.status = self.ai_status.clone();

            }
            Err(error) => {
                self.ai_status = format!(
                    "{} failed: {error}",
                    if let Some(kind) = self.ai_restoration {
                        kind.name()
                    } else if self.ai_is_denoise {
                        "AI denoise"
                    } else {
                        "AI upscale"
                    }
                );
                self.status = self.ai_status.clone();
            }
        }
    }

    fn compare_ai_variations(&mut self, ctx: &egui::Context) {
        let Some((_, a)) = self.ai_variations.get(self.ai_compare_a) else {
            return;
        };
        let Some((_, b)) = self.ai_variations.get(self.ai_compare_b) else {
            return;
        };
        if a.dimensions != b.dimensions {
            self.ai_status =
                "Cannot compare variations of different dimensions. Compare equal-size results."
                    .into();
            self.status = self.ai_status.clone();
            return;
        }
        let (a, b) = (a.clone(), b.clone());
        let folder = std::env::temp_dir().join(format!(
            "pixelforge_compare_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        if let Err(e) = std::fs::create_dir_all(&folder) {
            self.ai_status = format!("Cannot prepare comparison: {e}");
            return;
        }
        let result = ai_upscale::UpscaleResult {
            work_dir: folder,
            ai_path: b.ai_path.clone(),
            baseline_path: a.ai_path.clone(),
            preview: b.preview.clone(),
            baseline_preview: a.preview.clone(),
            dimensions: b.dimensions,
        };
        self.ai_result_size = result.dimensions;
        let color = ColorImage::from_rgba_unmultiplied(
            [
                result.baseline_preview.width() as usize,
                result.baseline_preview.height() as usize,
            ],
            result.baseline_preview.as_raw(),
        );
        self.ai_baseline_texture =
            Some(ctx.load_texture("variation_a", color, TextureOptions::LINEAR));
        self.ai_result = Some(std::sync::Arc::new(result));
        self.ai_comparison = true;
        self.ai_result_path = None;
        self.ai_detail_rx = None;
        self.ai_detail_key = None;
        self.ai_detail_rect = None;
        self.ai_detail_ai_texture = None;
        self.ai_detail_lanczos_texture = None;
        self.ai_decode_cache = std::sync::Arc::new(std::sync::Mutex::new(None));
        self.refresh_ai_preview(ctx);
        self.ai_show_result = true;
        self.ai_status = "Comparing saved AI variations".into();
    }

    fn refresh_ai_preview(&mut self, ctx: &egui::Context) {
        let Some(result) = &self.ai_result else {
            return;
        };

        let Ok(blended) =
            ai_upscale::blend_images(&result.preview, &result.baseline_preview, self.ai_strength)
        else {
            return;
        };

        let size = [blended.width() as usize, blended.height() as usize];
        let color = ColorImage::from_rgba_unmultiplied(size, blended.as_raw());

        self.ai_result_texture =
            Some(ctx.load_texture("ai_strength_preview", color, TextureOptions::LINEAR));

        self.ai_inspect_texture = None;
        self.ai_inspect_size = size;

        // Strength changes invalidate the native-resolution blend.
        self.ai_detail_key = None;
        self.ai_detail_rect = None;
        self.ai_detail_ai_texture = None;
        self.ai_detail_lanczos_texture = None;

        ctx.request_repaint();
    }

    fn save_ai_result(&mut self, ctx: &egui::Context) {
        if self.ai_running || self.ai_saving || self.exporting {
            return;
        }

        let Some(result) = self.ai_result.clone() else {
            return;
        };

        let stem = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into());

        let scale = self
            .original_pixels
            .as_ref()
            .map(|image| {
                if image.width().checked_mul(2) == Some(result.dimensions[0]) {
                    2
                } else {
                    4
                }
            })
            .unwrap_or(4);

        let suggested = if let Some(kind) = self.ai_restoration {
            format!("{stem}_AI_{}.png", kind.name())
        } else if self.ai_is_denoise {
            format!("{stem}_AI_Denoised.png")
        } else {
            format!("{stem}_AI_{scale}x.png")
        };

        let Some(mut destination) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name(&suggested)
            .save_file()
        else {
            return;
        };

        destination.set_extension("png");

        if destination.exists() {
            self.ai_status = "Filename already exists. Choose another.".into();
            return;
        }

        let strength = self.ai_strength;
        let (tx, rx) = std::sync::mpsc::channel();

        self.ai_save_rx = Some(rx);
        self.ai_saving = true;
        self.ai_status = "Saving full-resolution AI result...".into();

        let ctx = ctx.clone();

        std::thread::spawn(move || {
            // Arc retains temporary files until export finishes.
            let saved = ai_upscale::save_blended(
                &result.ai_path,
                &result.baseline_path,
                strength,
                &destination,
            );

            let _ = tx.send(saved);
            ctx.request_repaint();
        });
    }

    fn poll_ai_save(&mut self, ctx: &egui::Context) {
        if !self.ai_saving {
            return;
        }

        let result = self.ai_save_rx.as_ref().and_then(|rx| rx.try_recv().ok());

        if let Some(result) = result {
            self.ai_saving = false;
            self.ai_save_rx = None;

            match result {
                Ok(path) => {
                    self.ai_status = format!("Saved: {}", path.display());
                    self.ai_result_path = Some(path);
                }
                Err(error) => {
                    self.ai_status = format!("Save failed: {error}");
                }
            }

            self.status = self.ai_status.clone();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn show_ai_result(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if !self.ai_show_result {
            return;
        }

        if self.ai_result_texture.is_none() {
            return;
        }

        let Some(texture_id) = self.ai_result_texture.as_ref().map(|t| t.id()) else {
            return;
        };

        let source_size = Vec2::new(self.ai_result_size[0] as f32, self.ai_result_size[1] as f32);

        let limited = self.ai_inspect_size[0] as u32 != self.ai_result_size[0]
            || self.ai_inspect_size[1] as u32 != self.ai_result_size[1];

        ui.horizontal(|ui| {
            ui.heading("AI Result / Variations");

            if ui
                .add_enabled(
                    !self.ai_saving && self.ai_result.is_some(),
                    egui::Button::new("Save Result..."),
                )
                .clicked()
            {
                self.save_ai_result(ctx);
            }

            if ui.add_enabled(self.ai_result.is_some() && self.edit_versions.len() < project::MAX_VERSIONS,
                egui::Button::new("Add AI Result to History")).clicked() {
                let path = self.ai_result.as_ref().map(|result| result.ai_path.clone());
                if let Some(path) = path {
                    match image::open(&path) {
                        Ok(image) => {
                            self.edit_versions.push(EditVersion {
                                global: adjustments::Adjustments::default(),
                                local: adjustments::Adjustments::default(),
                                mask: None,
                                raster: Some(image.to_rgba8()),
                            });
                            self.project_dirty = true;
                            self.status = "AI result added to image history".into();
                        }
                        Err(error) => self.status = format!("Cannot capture AI result: {error}"),
                    }
                }
            }

            if self.ai_saving {
                ui.spinner();
                crate::interaction::label(ui,"Saving PNG...");
            }

            if ui.button("Back to Editor").clicked() {
                self.ai_show_result = false;
                self.ai_divider_dragging = false;
            }
        });

        ui.separator();
        if !self.ai_variations.is_empty() {
            ui.horizontal_wrapped(|ui| {
                crate::interaction::label(ui,"Compare AI variations:");
                egui::ComboBox::from_id_salt("ai_variation_a")
                    .selected_text(
                        self.ai_variations
                            .get(self.ai_compare_a)
                            .map(|v| v.0.as_str())
                            .unwrap_or("A"),
                    )
                    .show_ui(ui, |ui| {
                        for (i, (name, _)) in self.ai_variations.iter().enumerate() {
                            ui.selectable_value(&mut self.ai_compare_a, i, name);
                        }
                    });
                crate::interaction::label(ui,"vs");
                egui::ComboBox::from_id_salt("ai_variation_b")
                    .selected_text(
                        self.ai_variations
                            .get(self.ai_compare_b)
                            .map(|v| v.0.as_str())
                            .unwrap_or("B"),
                    )
                    .show_ui(ui, |ui| {
                        for (i, (name, _)) in self.ai_variations.iter().enumerate() {
                            ui.selectable_value(&mut self.ai_compare_b, i, name);
                        }
                    });
                if ui.button("Compare Selected").clicked() {
                    self.compare_ai_variations(ctx);
                }
            });
            crate::interaction::small(ui,"AI results are retained in memory for this image until you open another image or close PixelForge.");
        }

        {
            ui.horizontal_wrapped(|ui| {
                crate::interaction::label(ui,format!(
                    "Output: {} x {}",
                    self.ai_result_size[0], self.ai_result_size[1]
                ));

                ui.separator();

                if ui.button("Fit").clicked() {
                    self.ai_inspect_fit = true;
                    self.ai_inspect_pan = Vec2::ZERO;
                }

                if ui.button("100%").clicked() {
                    self.ai_inspect_fit = false;
                    self.ai_inspect_zoom = 1.0;
                    self.ai_inspect_pan = Vec2::ZERO;
                }

                if ui.button("-").clicked() {
                    self.ai_inspect_fit = false;
                    self.ai_inspect_zoom = (self.ai_inspect_zoom / 1.25).clamp(0.05, 16.0);
                }

                if ui.button("+").clicked() {
                    self.ai_inspect_fit = false;
                    self.ai_inspect_zoom = (self.ai_inspect_zoom * 1.25).clamp(0.05, 16.0);
                }

                if ui.button("Reset").clicked() {
                    self.ai_inspect_fit = true;
                    self.ai_inspect_pan = Vec2::ZERO;
                }

                ui.separator();

                crate::interaction::label(ui,format!("{:.0}%", self.ai_inspect_zoom * 100.0));
            });

            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.ai_comparison,
                    if let Some(kind) = self.ai_restoration {
                        match kind {
                            ai_restoration::Operation::Sharpen => "Compare Original / Sharpened",
                            ai_restoration::Operation::Deblur => "Compare Original / Deblurred",
                        }
                    } else if self.ai_is_denoise {
                        "Compare Original / Denoised"
                    } else {
                        "Compare Lanczos / Result"
                    },
                );

                if self.ai_comparison {
                    ui.add(
                        numeric::Slider::new(&mut self.ai_comparison_position, 0.0..=1.0)
                            .text("Divider"),
                    );
                }
            });

            if let Some(path) = &self.ai_result_path {
                crate::interaction::label(ui,format!("Last saved: {}", path.display()));
            } else {
                crate::interaction::label(ui,"Temporary result - not saved");
            }

            if limited {
                crate::interaction::small(ui,
                    "Adaptive-resolution inspection works at every zoom level. \
                     The visible detail region loads in the background; \
                     the fast preview remains underneath.",
                );
            }

            // Keep the inspection status area at a fixed height.
            // Changing status text must not resize the preview canvas.
            ui.horizontal(|ui| {
                ui.set_min_height(20.0);

                let message = if self.ai_result.is_none() {
                    "Preview mode"
                } else if self.ai_detail_rx.is_some() {
                    "Loading native-resolution detail..."
                } else if self.ai_detail_status.contains("inspection")
                    || self.ai_detail_status.contains("Cannot")
                    || self.ai_detail_status.contains("decoder limit")
                    || self.ai_detail_status.contains("cache lock")
                    || self.ai_detail_status.contains("worker disconnected")
                {
                    self.ai_detail_status.as_str()
                } else if self.ai_detail_ai_texture.is_some() {
                    "Native-resolution detail ready"
                } else {
                    "Waiting for native-resolution detail"
                };

                crate::interaction::label_widget(ui,egui::Label::new(egui::RichText::new(message).small()).truncate());
            });

            crate::interaction::small(ui,format!(
                "Mouse wheel: zoom | {}: pan | Fit: whole image | 100%: image pixel scale",
                match self.controls.pan {
                    PanButton::Middle => "Middle-drag",
                    PanButton::Left => "Left-drag",
                    PanButton::SpaceLeft => "Space + Left-drag",
                }
            ));

            ui.separator();

            let available = ui.available_size().max(Vec2::splat(64.0));
            let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

            let fit = (rect.width() / source_size.x.max(1.0))
                .min(rect.height() / source_size.y.max(1.0))
                .min(1.0)
                .max(0.001);

            if self.ai_inspect_fit {
                self.ai_inspect_zoom = fit;
                self.ai_inspect_pan = Vec2::ZERO;
            }

            if response.hovered() {
                let wheel = ui.input(|i| i.raw_scroll_delta.y);

                if wheel.abs() > 0.01 {
                    self.ai_inspect_fit = false;

                    let old_zoom = self.ai_inspect_zoom;
                    let new_zoom = (old_zoom * (wheel * 0.0015).exp()).clamp(0.05, 16.0);

                    if let Some(pointer) = response.hover_pos() {
                        let center = rect.center();
                        let relative = pointer - center - self.ai_inspect_pan;

                        if old_zoom > 0.0 {
                            self.ai_inspect_pan -= relative * (new_zoom / old_zoom - 1.0);
                        }
                    }

                    self.ai_inspect_zoom = new_zoom;
                    ctx.request_repaint();
                }
            }

            // The divider uses the primary mouse button.
            // Capture a drag only when it begins close to
            // the visible comparison divider.
            if self.ai_comparison && self.ai_baseline_texture.is_some() {
                let divider_x = rect.left() + rect.width() * self.ai_comparison_position;

                if response
                    .hover_pos()
                    .map(|pos| (pos.x - divider_x).abs() <= 14.0)
                    .unwrap_or(false)
                    || self.ai_divider_dragging
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }

                if response.drag_started_by(egui::PointerButton::Primary) {
                    let near_divider = response
                        .interact_pointer_pos()
                        .map(|pos| (pos.x - divider_x).abs() <= 14.0)
                        .unwrap_or(false);

                    self.ai_divider_dragging = near_divider;
                }

                if self.ai_divider_dragging {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.ai_comparison_position =
                            ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);

                        ctx.request_repaint();
                    }
                }
            }

            if response.drag_stopped() {
                self.ai_divider_dragging = false;
            }

            // Match the main editor's configured pan button.
            let pan_active = match self.controls.pan {
                PanButton::Middle => response.dragged_by(egui::PointerButton::Middle),
                PanButton::Left => response.dragged_by(egui::PointerButton::Primary),
                PanButton::SpaceLeft => {
                    let space_held = ui.input(|i| i.key_down(egui::Key::Space));
                    space_held && response.dragged_by(egui::PointerButton::Primary)
                }
            };

            if pan_active && !self.ai_divider_dragging {
                self.ai_inspect_fit = false;
                self.ai_inspect_pan += ui.input(|i| i.pointer.delta());
                ctx.request_repaint();
            }

            let draw_size = source_size * self.ai_inspect_zoom;
            let center = rect.center() + self.ai_inspect_pan;

            let image_rect = Rect::from_center_size(center, draw_size);

            // Request a native-resolution crop for the visible image region.
            // The ordinary preview remains available while it loads.
            if self.ai_inspect_zoom > 0.0 {
                if let Some(result) = self.ai_result.as_ref() {
                    let visible = rect.intersect(image_rect);

                    if visible.width() > 1.0 && visible.height() > 1.0 {
                        let zoom = self.ai_inspect_zoom.max(0.001);

                        let source_x = ((visible.left() - image_rect.left()) / zoom)
                            .floor()
                            .max(0.0) as u32;

                        let source_y =
                            ((visible.top() - image_rect.top()) / zoom).floor().max(0.0) as u32;

                        let source_w = (visible.width() / zoom).ceil() as u32;
                        let source_h = (visible.height() / zoom).ceil() as u32;

                        let image_w = self.ai_result_size[0];
                        let image_h = self.ai_result_size[1];

                        // A bounded 2048 x 2048 native crop limits GPU memory.
                        // Center it on the visible area, with some extra margin.
                        let crop_w = source_w.saturating_add(256).min(image_w).max(1);

                        let crop_h = source_h.saturating_add(256).min(image_h).max(1);

                        let center_x = source_x.saturating_add(source_w / 2);
                        let center_y = source_y.saturating_add(source_h / 2);

                        let max_x = image_w.saturating_sub(crop_w);
                        let max_y = image_h.saturating_sub(crop_h);

                        // Align crop origins to a 64-pixel grid.
                        // Small movements reuse the same inspection crop.
                        let crop_x = (center_x.saturating_sub(crop_w / 2) / 64 * 64).min(max_x);

                        let crop_y = (center_y.saturating_sub(crop_h / 2) / 64 * 64).min(max_y);

                        let crop = [crop_x, crop_y, crop_w, crop_h];
                        let strength_key =
                            (self.ai_strength.clamp(0.0, 1.0) * 1000.0).round() as u32;

                        let key = (crop, strength_key);

                        if self.ai_detail_rx.is_none() && self.ai_detail_key != Some(key) {
                            let retained = std::sync::Arc::clone(result);
                            let decode_cache = std::sync::Arc::clone(&self.ai_decode_cache);

                            let (tx, rx) = std::sync::mpsc::channel();

                            self.ai_detail_rx = Some(rx);
                            self.ai_detail_key = Some(key);
                            self.ai_detail_status = "Loading native pixels...".into();

                            let repaint = ctx.clone();

                            std::thread::spawn(move || {
                                let loaded = (|| -> Result<
                                    ([u32; 4], u32, image::RgbaImage, image::RgbaImage),
                                    String
                                > {
                                    // Maximum retained decoded-image cache:
                                    // 4 GiB for the AI/Lanczos image pair.
                                    const CACHE_LIMIT: u64 = 4 * 1024 * 1024 * 1024;

                                    let bytes_needed =
                                        retained.dimensions[0] as u64
                                            * retained.dimensions[1] as u64
                                            * 8;

                                    let cached = {
                                        let guard = decode_cache
                                            .lock()
                                            .map_err(|_| "Inspection cache lock failed")?;

                                        guard.clone()
                                    };

                                    let (ai, baseline) = if let Some(pair) = cached {
                                        pair
                                    } else {
                                        let ai = std::sync::Arc::new(
                                            load_inspection_png(&retained.ai_path, "AI inspection")?
                                        );

                                        let baseline = std::sync::Arc::new(
                                            load_inspection_png(&retained.baseline_path, "Lanczos inspection")?
                                        );

                                        if ai.dimensions() != baseline.dimensions() {
                                            return Err(
                                                "Inspection image sizes differ".into()
                                            );
                                        }

                                        if bytes_needed <= CACHE_LIMIT {
                                            let mut guard = decode_cache
                                                .lock()
                                                .map_err(|_| {
                                                    "Inspection cache lock failed"
                                                })?;

                                            *guard = Some((
                                                std::sync::Arc::clone(&ai),
                                                std::sync::Arc::clone(&baseline),
                                            ));
                                        }

                                        (ai, baseline)
                                    };

                                    // crop_imm requires GenericImageView.
                                    // Dereference Arc to the underlying image.

                                    let ai_crop = image::imageops::crop_imm(
                                        ai.as_ref(), crop[0], crop[1], crop[2], crop[3]
                                    ).to_image();

                                    let baseline_crop = image::imageops::crop_imm(
                                        baseline.as_ref(), crop[0], crop[1], crop[2], crop[3]
                                    ).to_image();

                                    let blended = ai_upscale::blend_images(
                                        &ai_crop,
                                        &baseline_crop,
                                        strength_key as f32 / 1000.0,
                                    )?;

                                    // Downsample full-resolution source pixels
                                    // into a bounded display texture.
                                    let max_side = crop[2].max(crop[3]);

                                    let (display_ai, display_lanczos) =
                                        if max_side > 2048 {
                                            let factor = 2048.0 / max_side as f64;

                                            let width =
                                                ((crop[2] as f64 * factor).round() as u32).max(1);

                                            let height =
                                                ((crop[3] as f64 * factor).round() as u32).max(1);

                                            (
                                                image::imageops::resize(
                                                    &blended,
                                                    width,
                                                    height,
                                                    image::imageops::FilterType::Lanczos3,
                                                ),
                                                image::imageops::resize(
                                                    &baseline_crop,
                                                    width,
                                                    height,
                                                    image::imageops::FilterType::Lanczos3,
                                                ),
                                            )
                                        } else {
                                            (blended, baseline_crop)
                                        };

                                    Ok((
                                        crop,
                                        strength_key,
                                        display_ai,
                                        display_lanczos,
                                    ))
                                })();

                                let _ = tx.send(loaded);
                                repaint.request_repaint();
                            });
                        }
                    }
                }
            }

            // Poll without blocking the editor.
            let detail_message = self
                .ai_detail_rx
                .as_ref()
                .and_then(|rx| match rx.try_recv() {
                    Ok(value) => Some(value),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Inspection worker disconnected".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                });

            if let Some(message) = detail_message {
                self.ai_detail_rx = None;

                match message {
                    Ok((crop, strength_key, ai_pixels, lanczos_pixels)) => {
                        // A strength or viewport change may have occurred
                        // while the worker was decoding the PNG files.
                        if self.ai_detail_key == Some((crop, strength_key))
                            && strength_key
                                == (self.ai_strength.clamp(0.0, 1.0) * 1000.0).round() as u32
                        {
                            let size = [ai_pixels.width() as usize, ai_pixels.height() as usize];

                            let ai_color =
                                ColorImage::from_rgba_unmultiplied(size, ai_pixels.as_raw());

                            let baseline_color =
                                ColorImage::from_rgba_unmultiplied(size, lanczos_pixels.as_raw());

                            self.ai_detail_ai_texture = Some(ctx.load_texture(
                                "ai_native_inspection",
                                ai_color,
                                TextureOptions::LINEAR,
                            ));

                            self.ai_detail_lanczos_texture = Some(ctx.load_texture(
                                "lanczos_native_inspection",
                                baseline_color,
                                TextureOptions::LINEAR,
                            ));

                            self.ai_detail_rect = Some(crop);
                            self.ai_detail_status = "Native-resolution detail ready".into();
                        }
                    }
                    Err(error) => {
                        self.ai_detail_key = None;
                        self.ai_detail_status = error;
                    }
                }
            }

            if self.ai_detail_rx.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }

            let painter = ui.painter_at(rect);

            painter.rect_filled(rect, 0.0, Color32::from_gray(30));

            painter.image(
                texture_id,
                image_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            // Overlay native AI pixels over the fast preview.
            if self.ai_inspect_zoom > 0.0 {
                if let (Some(crop), Some(detail)) =
                    (self.ai_detail_rect, &self.ai_detail_ai_texture)
                {
                    let detail_rect = Rect::from_min_size(
                        image_rect.min
                            + Vec2::new(
                                crop[0] as f32 * self.ai_inspect_zoom,
                                crop[1] as f32 * self.ai_inspect_zoom,
                            ),
                        Vec2::new(
                            crop[2] as f32 * self.ai_inspect_zoom,
                            crop[3] as f32 * self.ai_inspect_zoom,
                        ),
                    );

                    painter.image(
                        detail.id(),
                        detail_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }

            if self.ai_comparison {
                if let Some(baseline) = &self.ai_baseline_texture {
                    let divider_x = rect.left() + rect.width() * self.ai_comparison_position;

                    let left_clip =
                        Rect::from_min_max(rect.min, Pos2::new(divider_x, rect.bottom()));

                    let comparison_painter = painter.with_clip_rect(left_clip);

                    comparison_painter.image(
                        baseline.id(),
                        image_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );

                    // Use the same crop coordinates on the Lanczos side.
                    if self.ai_inspect_zoom > 0.0 {
                        if let (Some(crop), Some(detail)) =
                            (self.ai_detail_rect, &self.ai_detail_lanczos_texture)
                        {
                            let detail_rect = Rect::from_min_size(
                                image_rect.min
                                    + Vec2::new(
                                        crop[0] as f32 * self.ai_inspect_zoom,
                                        crop[1] as f32 * self.ai_inspect_zoom,
                                    ),
                                Vec2::new(
                                    crop[2] as f32 * self.ai_inspect_zoom,
                                    crop[3] as f32 * self.ai_inspect_zoom,
                                ),
                            );

                            comparison_painter.image(
                                detail.id(),
                                detail_rect,
                                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }

                    painter.line_segment(
                        [
                            Pos2::new(divider_x, rect.top()),
                            Pos2::new(divider_x, rect.bottom()),
                        ],
                        egui::Stroke::new(2.0, Color32::LIGHT_BLUE),
                    );

                    painter.text(
                        Pos2::new(rect.left() + 8.0, rect.top() + 8.0),
                        egui::Align2::LEFT_TOP,
                        if self.ai_is_denoise || self.ai_restoration.is_some() {
                            "Original"
                        } else {
                            "Lanczos"
                        },
                        egui::TextStyle::Body.resolve(ui.style()),
                        Color32::WHITE,
                    );

                    painter.text(
                        Pos2::new(rect.right() - 8.0, rect.top() + 8.0),
                        egui::Align2::RIGHT_TOP,
                        if let Some(kind) = self.ai_restoration {
                            match kind {
                                ai_restoration::Operation::Sharpen => "Sharpened",
                                ai_restoration::Operation::Deblur => "Deblurred",
                            }
                        } else if self.ai_is_denoise {
                            "Denoised"
                        } else {
                            "AI Result"
                        },
                        egui::TextStyle::Body.resolve(ui.style()),
                        Color32::WHITE,
                    );
                }
            }
        }
    }
    fn run_auto_enhance(&mut self, ctx: &egui::Context) {
        let Some(source) = &self.preview_source else {
            self.status = "Open an image before Auto Enhance".into();
            return;
        };

        // Analyze the unmodified preview, not the histogram of
        // previously applied adjustments.
        let hist = histogram::Histogram::from_image(source);

        let Some(plan) = auto_enhance::EnhancePlan::from_histogram(&hist) else {
            self.status = "No image data to enhance".into();
            return;
        };

        let old = (
            self.exposure,
            self.shadows,
            self.highlights,
            self.temperature,
            self.tint,
            self.vibrance,
        );

        self.exposure = plan.exposure;
        self.shadows = plan.shadows;
        self.highlights = plan.highlights;
        self.temperature = plan.temperature;
        self.tint = plan.tint;
        self.vibrance = plan.vibrance;

        let new = (
            self.exposure,
            self.shadows,
            self.highlights,
            self.temperature,
            self.tint,
            self.vibrance,
        );

        if old != new {
            self.project_dirty = true;
            self.update_processed_image(ctx);
            self.status = "Auto Enhance applied - adjustments remain editable".into();
        } else if plan.has_changes() {
            self.status = "Auto Enhance settings already applied".into();
        } else {
            self.status = "Image is already within conservative enhancement targets".into();
        }
    }
    fn show_image_analysis(&mut self, ui: &mut egui::Ui) {
        let Some(report) = analysis::Analysis::from_histogram(&self.histogram) else {
            crate::interaction::label(ui,"No image data available.");
            return;
        };

        crate::interaction::strong(ui,"Exposure");
        crate::interaction::strong(ui,report.exposure);

        ui.add_space(6.0);

        crate::interaction::label(ui,format!(
            "Average brightness: {:.0} / 255",
            report.mean_luminance
        ));

        ui.separator();

        crate::interaction::strong(ui,"Clipping");

        crate::interaction::label(ui,format!("Near-black pixels: {:.2}%", report.shadow_clipping));

        crate::interaction::label(ui,format!(
            "Near-white pixels: {:.2}%",
            report.highlight_clipping
        ));

        ui.separator();

        crate::interaction::strong(ui,"Tonal Distribution");

        crate::interaction::label(ui,format!("Shadows: {:.1}%", report.shadows));

        crate::interaction::label(ui,format!("Midtones: {:.1}%", report.midtones));

        crate::interaction::label(ui,format!("Highlights: {:.1}%", report.highlights));

        ui.separator();

        crate::interaction::strong(ui,"Color Balance");
        crate::interaction::strong(ui,report.color_balance);

        crate::interaction::small(ui,format!(
            "Average RGB: {:.0} / {:.0} / {:.0}",
            report.red_mean, report.green_mean, report.blue_mean
        ));

        ui.separator();

        crate::interaction::strong(ui,"Recommendations");

        egui::ScrollArea::vertical()
            .id_salt("pixelforge_recommendations_scroll")
            .max_height(110.0)
            .min_scrolled_height(110.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_height(110.0);

                for recommendation in &report.recommendations {
                    ui.add_space(4.0);
                    crate::interaction::label(ui,format!("• {recommendation}"));
                }
            });

        ui.add_space(6.0);
        crate::interaction::small(ui,
            "Analysis uses the current processed preview. \
             Results are guidance, not definitive image-quality judgments.",
        );
    }
    fn show_histogram(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.histogram_mode,
                histogram::HistogramMode::Rgb,
                "RGB",
            );

            ui.selectable_value(
                &mut self.histogram_mode,
                histogram::HistogramMode::Luminance,
                "Luminance",
            );
        });

        ui.add_space(4.0);
        self.histogram.show(ui, self.histogram_mode);

        ui.horizontal(|ui| {
            crate::interaction::small(ui,"Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::interaction::small(ui,"Highlights");
            });
        });

        crate::interaction::small(ui,"Based on the current image preview");
    }
    fn show_adjustments(&mut self, ui: &mut egui::Ui) {
        // Temporarily bind the existing sliders to the selection adjustment bank.
        // Restore global values before rendering the preview.
        if self.selection_adjustment_target {
            std::mem::swap(
                &mut self.brightness,
                &mut self.selection_adjustments.brightness,
            );
            std::mem::swap(&mut self.contrast, &mut self.selection_adjustments.contrast);
            std::mem::swap(&mut self.exposure, &mut self.selection_adjustments.exposure);
            std::mem::swap(
                &mut self.saturation,
                &mut self.selection_adjustments.saturation,
            );
            std::mem::swap(
                &mut self.temperature,
                &mut self.selection_adjustments.temperature,
            );
            std::mem::swap(&mut self.tint, &mut self.selection_adjustments.tint);
            std::mem::swap(
                &mut self.highlights,
                &mut self.selection_adjustments.highlights,
            );
            std::mem::swap(&mut self.shadows, &mut self.selection_adjustments.shadows);
            std::mem::swap(&mut self.whites, &mut self.selection_adjustments.whites);
            std::mem::swap(&mut self.blacks, &mut self.selection_adjustments.blacks);
            std::mem::swap(&mut self.vibrance, &mut self.selection_adjustments.vibrance);
            std::mem::swap(&mut self.gamma, &mut self.selection_adjustments.gamma);
        }
        if !self.generation_workspace {
        crate::interaction::strong(ui,"Version History");
        ui.scope(|ui| {
            crate::interaction::label(ui,"Save a version of your edits. AI results are stored as image snapshots automatically.");
            if ui
                .add_enabled(
                    self.edit_versions.len() < project::MAX_VERSIONS,
                    egui::Button::new("Save Current Version"),
                )
                .clicked()
            {
                let current = self.current_adjustments();
                let (global, local) = if self.selection_adjustment_target {
                    (self.selection_adjustments, current)
                } else {
                    (current, self.selection_adjustments)
                };
                self.edit_versions.push(EditVersion {
                    global, local, mask: self.selection_mask.clone(), raster: None,
                });
                self.version_comparison = false;
                self.version_texture_a = None;
                self.version_texture_b = None;
                self.project_dirty = true;
                self.status = "Version saved to project history".into();
            }
            let mut restore = None;
            for index in 0..self.edit_versions.len() {
                ui.horizontal(|ui| {
                    let label = if self.edit_versions[index].raster.is_some() {
                        format!("Version {} — AI Image", index + 1)
                    } else if index == 0 {
                        "Version 1 — Original / Edits".to_owned()
                    } else {
                        format!("Version {} — Edits", index + 1)
                    };
                    crate::interaction::label(ui,label);
                    if ui.button("Restore").clicked() {
                        restore = Some(index);
                    }
                });
            }
            if let Some(index) = restore {
                let version = self.edit_versions[index].clone();
                let active = if self.selection_adjustment_target {
                    version.local
                } else {
                    version.global
                };
                self.set_current_adjustments(active);
                self.selection_adjustments = if self.selection_adjustment_target {
                    version.global
                } else {
                    version.local
                };
                self.selection_mask = version.mask;
                self.active_raster_version = (0..=index).rev()
                    .find(|&i| self.edit_versions[i].raster.is_some());
                self.activate_version_source(ui.ctx());
                self.version_comparison = false;
                self.project_dirty = true;
                self.update_processed_image(ui.ctx());
                self.refresh_selection_overlay(ui.ctx());
            }
            if self.edit_versions.len() >= 2 {
                ui.separator();
                crate::interaction::label(ui,"Compare saved versions");
                let last = self.edit_versions.len() - 1;
                self.compare_version_a = self.compare_version_a.min(last);
                self.compare_version_b = self.compare_version_b.min(last);
                egui::ComboBox::from_label("Left version")
                    .selected_text(format!("Version {}", self.compare_version_a + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..self.edit_versions.len() {
                            ui.selectable_value(&mut self.compare_version_a, i, format!("Version {}", i + 1));
                        }
                    });
                egui::ComboBox::from_label("Right version")
                    .selected_text(format!("Version {}", self.compare_version_b + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..self.edit_versions.len() {
                            ui.selectable_value(&mut self.compare_version_b, i, format!("Version {}", i + 1));
                        }
                    });
                if ui.button("Compare Versions").clicked() {
                    self.start_version_comparison(ui.ctx());
                }
                if self.version_comparison {
                    if ui.button("Stop Comparing").clicked() {
                        self.version_comparison = false;
                    }
                    crate::interaction::label(ui,"Drag the divider on the image to compare versions.");
                }
            }
            crate::interaction::small(ui,"Versions are saved with your .pxf project.");
        });
        }
        let mut preview_needs_refresh = false;
        let before_adjustments = [
            self.brightness,
            self.contrast,
            self.exposure,
            self.saturation,
            self.temperature,
            self.tint,
            self.highlights,
            self.shadows,
            self.whites,
            self.blacks,
            self.vibrance,
            self.gamma,
        ];

        let before_comparison = (self.comparison_enabled, self.comparison_position);
        ui.horizontal(|ui| {
            let can_reset = self.brightness != 0.0
                || self.contrast != 0.0
                || self.exposure != 0.0
                || self.saturation != 0.0
                || self.temperature != 0.0
                || self.tint != 0.0
                || self.highlights != 0.0
                || self.shadows != 0.0
                || self.whites != 0.0
                || self.blacks != 0.0
                || self.vibrance != 0.0
                || self.gamma != 0.0;

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
                self.highlights = 0.0;
                self.shadows = 0.0;
                self.whites = 0.0;
                self.blacks = 0.0;
                self.vibrance = 0.0;
                self.gamma = 0.0;
                preview_needs_refresh = true;
            }
        });

        ui.horizontal(|ui| {
            crate::interaction::label(ui,"Adjustment target:");
            ui.selectable_value(&mut self.selection_adjustment_target, false, "Global");
            ui.selectable_value(&mut self.selection_adjustment_target, true, "Selection");
        });
        if let Some(mask) = &self.selection_mask {
            let active_pixels = mask.pixels().iter().filter(|&&alpha| alpha > 0).count();
            if active_pixels > 0 {
                let label = self
                    .active_saved_selection
                    .and_then(|index| self.saved_selections.get(index))
                    .map(|(name, _)| name.as_str())
                    .unwrap_or("Unsaved selection");
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "● Selection active");
                    crate::interaction::label(ui,label);
                });
                if ui
                    .button("Clear Selection")
                    .on_hover_text("Clear the active mask without deleting saved selections")
                    .clicked()
                {
                    self.selection_checkpoint();
                    if let Some(mask) = self.selection_mask.as_mut() {
                        mask.clear();
                    }
                    self.active_saved_selection = None;
                    self.selected_saved_selection = None;
                    self.selection_name_input.clear();
                    self.selection_last_point = None;
                    self.project_dirty = true;
                    self.refresh_selection_overlay(ui.ctx());
                    self.update_processed_image(ui.ctx());
                }
                if self.selection_adjustment_target {
                    crate::interaction::small(ui,"Adjustments affect the active mask. Drag the before/after divider to compare.");
                }
            } else {
                crate::interaction::label(ui,"Selection mask is empty.");
            }
        } else {
            crate::interaction::label(ui,"No active selection mask.");
            if self.selection_adjustment_target {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Paint or load a selection mask first.",
                );
            }
        }

        ui.separator();

        let mut changed = false;

        crate::interaction::strong(ui,"Light & Tone");
        ui.scope(|ui| {
                crate::interaction::label(ui,"Exposure (EV)");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.exposure, -3.0..=3.0).step_by(0.1))
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
                crate::interaction::label(ui,"Brightness");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.brightness, -100.0..=100.0))
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
                crate::interaction::label(ui,"Contrast");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.contrast, -100.0..=100.0))
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
                crate::interaction::label(ui,"Gamma");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.gamma, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.gamma != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.gamma = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
            });

        crate::interaction::strong(ui,"Tonal Range");
        ui.scope(|ui| {
                crate::interaction::label(ui,"Highlights");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.highlights, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.highlights != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.highlights = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
                crate::interaction::label(ui,"Shadows");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.shadows, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.shadows != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.shadows = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
                crate::interaction::label(ui,"Whites");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.whites, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.whites != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.whites = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
                crate::interaction::label(ui,"Blacks");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.blacks, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.blacks != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.blacks = 0.0;
                        self.vibrance = 0.0;
                        self.gamma = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
            });

        crate::interaction::strong(ui,"Color");
        ui.scope(|ui| {
                crate::interaction::label(ui,"Temperature (Cool / Warm)");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.temperature, -100.0..=100.0))
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
                crate::interaction::label(ui,"Tint (Green / Magenta)");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.tint, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.tint != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.tint = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
                crate::interaction::label(ui,"Saturation");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.saturation, -100.0..=100.0))
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
                crate::interaction::label(ui,"Vibrance");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(numeric::Slider::new(&mut self.vibrance, -100.0..=100.0))
                        .changed();

                    if ui
                        .add_enabled(self.vibrance != 0.0, egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.vibrance = 0.0;
                        changed = true;
                    }
                });
                ui.add_space(6.0);
            });

        if changed {
            preview_needs_refresh = true;
        }

        let after_adjustments = [
            self.brightness,
            self.contrast,
            self.exposure,
            self.saturation,
            self.temperature,
            self.tint,
            self.highlights,
            self.shadows,
            self.whites,
            self.blacks,
            self.vibrance,
            self.gamma,
        ];

        if before_adjustments != after_adjustments
            || before_comparison != (self.comparison_enabled, self.comparison_position)
        {
            self.project_dirty = true;
        }
        if self.selection_adjustment_target {
            std::mem::swap(
                &mut self.brightness,
                &mut self.selection_adjustments.brightness,
            );
            std::mem::swap(&mut self.contrast, &mut self.selection_adjustments.contrast);
            std::mem::swap(&mut self.exposure, &mut self.selection_adjustments.exposure);
            std::mem::swap(
                &mut self.saturation,
                &mut self.selection_adjustments.saturation,
            );
            std::mem::swap(
                &mut self.temperature,
                &mut self.selection_adjustments.temperature,
            );
            std::mem::swap(&mut self.tint, &mut self.selection_adjustments.tint);
            std::mem::swap(
                &mut self.highlights,
                &mut self.selection_adjustments.highlights,
            );
            std::mem::swap(&mut self.shadows, &mut self.selection_adjustments.shadows);
            std::mem::swap(&mut self.whites, &mut self.selection_adjustments.whites);
            std::mem::swap(&mut self.blacks, &mut self.selection_adjustments.blacks);
            std::mem::swap(&mut self.vibrance, &mut self.selection_adjustments.vibrance);
            std::mem::swap(&mut self.gamma, &mut self.selection_adjustments.gamma);
        }
        if preview_needs_refresh {
            self.version_comparison = false;
            self.version_texture_a = None;
            self.version_texture_b = None;
            self.comparison_enabled = false;
            self.update_processed_image(ui.ctx());
        }
    }
    fn selection_checkpoint(&mut self) {
        const MAX_HISTORY: usize = 20;

        if self.selection_undo.len() >= MAX_HISTORY {
            self.selection_undo.remove(0);
        }

        self.selection_undo.push(self.selection_mask.clone());
        self.selection_redo.clear();
    }

    fn selection_undo_action(&mut self, ctx: &egui::Context) {
        if let Some(previous) = self.selection_undo.pop() {
            self.selection_redo.push(self.selection_mask.clone());
            self.selection_mask = previous;
            self.selection_last_point = None;
            self.selection_stroke_active = false;
            self.project_dirty = true;
            self.refresh_selection_overlay(ctx);
        }
    }

    fn selection_redo_action(&mut self, ctx: &egui::Context) {
        if let Some(next) = self.selection_redo.pop() {
            self.selection_undo.push(self.selection_mask.clone());
            self.selection_mask = next;
            self.selection_last_point = None;
            self.selection_stroke_active = false;
            self.project_dirty = true;
            self.refresh_selection_overlay(ctx);
        }
    }
    fn refresh_selection_overlay(&mut self, ctx: &egui::Context) {
        // Any mask change invalidates the adjustment preview, including undo/redo and brush strokes.
        if self.preview_source.is_some() {
            self.update_processed_image(ctx);
        }
        let Some(mask) = self.selection_mask.as_ref() else {
            self.selection_texture = None;
            return;
        };

        let (width, height) = mask.dimensions();

        if width == 0 || height == 0 {
            self.selection_texture = None;
            return;
        }

        let scale = (1024.0 / width.max(height) as f64).min(1.0);
        let out_w = ((width as f64 * scale).round() as usize).max(1);
        let out_h = ((height as f64 * scale).round() as usize).max(1);

        let mut rgba = vec![0u8; out_w * out_h * 4];

        for y in 0..out_h {
            for x in 0..out_w {
                let sx = ((x as u64 * width as u64) / out_w as u64).min(width as u64 - 1) as u32;

                let sy = ((y as u64 * height as u64) / out_h as u64).min(height as u64 - 1) as u32;

                let alpha = mask.get(sx, sy).unwrap_or(0);
                let index = (y * out_w + x) * 4;

                rgba[index] = 255;
                rgba[index + 1] = 45;
                rgba[index + 2] = 55;
                rgba[index + 3] = ((alpha as u16 * 110) / 255) as u8;
            }
        }

        let image = ColorImage::from_rgba_unmultiplied([out_w, out_h], &rgba);

        self.selection_texture =
            Some(ctx.load_texture("selection_mask_overlay", image, TextureOptions::LINEAR));
    }
    fn show_selection_sidebar(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Selection Tools");
        ui.separator();

        if self.texture.is_none() {
            crate::interaction::label(ui,"Open a photograph to begin selecting.");
            return;
        }

        // Keep a separate Context handle so column
        // closures do not borrow the parent Ui.
        let ctx = ui.ctx().clone();
        theme::section(ui, "Saved Selections");

        if self.saved_selections.is_empty() {
            crate::interaction::small(ui,"No saved selections yet.");
        } else {
            let mut selected = self.selected_saved_selection;
            let mut activate = None;

            egui::ScrollArea::vertical()
                .id_salt("saved_selections")
                .max_height(130.0)
                .show(ui, |ui| {
                    for (index, (name, _)) in self.saved_selections.iter().enumerate() {
                        let response =
                            ui.selectable_label(self.active_saved_selection == Some(index), name);
                        if response.double_clicked() {
                            selected = Some(index);
                            activate = Some(index);
                        } else if response.clicked() {
                            selected = Some(index);
                        }
                    }
                });

            if selected != self.selected_saved_selection {
                self.selected_saved_selection = selected;
                if let Some(index) = selected {
                    self.selection_name_input = self.saved_selections[index].0.clone();
                }
            }

            if let Some(index) = activate {
                self.selection_checkpoint();
                self.selection_mask = Some(self.saved_selections[index].1.clone());
                self.active_saved_selection = Some(index);
                self.selection_last_point = None;
                self.refresh_selection_overlay(&ctx);
            }
        }

        crate::interaction::label(ui,"Selection name");
        interaction::singleline(ui,&mut self.selection_name_input);

        let name = self.selection_name_input.trim().to_owned();
        let valid_name = !name.is_empty() && name.len() <= 256;
        let has_mask = self.selection_mask.is_some();

        let name_available = !self
            .saved_selections
            .iter()
            .any(|(existing, _)| existing == &name);

        let has_saved = self
            .selected_saved_selection
            .is_some_and(|i| i < self.saved_selections.len());

        let ctx_for_library = ctx.clone();

        ui.columns(2, |columns| {
            let left = columns[0].available_width();
            let right = columns[1].available_width();

            if columns[0]
                .add_enabled(
                    valid_name && name_available && has_mask && self.saved_selections.len() < 128,
                    egui::Button::new("Save New").min_size(egui::vec2(left, 28.0)),
                )
                .clicked()
            {
                if let Some(mask) = self.selection_mask.clone() {
                    self.saved_selections.push((name.clone(), mask));
                    self.selected_saved_selection = None;
                    self.selection_name_input.clear();
                    self.active_saved_selection = None;
                    self.project_dirty = true;
                }
            }

            if columns[1]
                .add_enabled(
                    has_saved,
                    egui::Button::new("Load").min_size(egui::vec2(right, 28.0)),
                )
                .clicked()
            {
                if let Some(index) = self.selected_saved_selection {
                    self.selection_checkpoint();
                    self.selection_mask = Some(self.saved_selections[index].1.clone());
                    self.active_saved_selection = Some(index);
                    self.selection_last_point = None;
                    self.refresh_selection_overlay(&ctx_for_library);
                }
            }
        });

        ui.add_space(4.0);

        ui.columns(2, |columns| {
            let left = columns[0].available_width();
            let right = columns[1].available_width();

            if columns[0]
                .add_enabled(
                    has_saved && has_mask,
                    egui::Button::new("Update").min_size(egui::vec2(left, 28.0)),
                )
                .clicked()
            {
                if let (Some(index), Some(mask)) =
                    (self.selected_saved_selection, self.selection_mask.clone())
                {
                    self.saved_selections[index].1 = mask;
                    self.project_dirty = true;
                }
            }

            let rename_available =
                self.saved_selections
                    .iter()
                    .enumerate()
                    .all(|(index, (existing, _))| {
                        Some(index) == self.selected_saved_selection || existing != &name
                    });

            if columns[1]
                .add_enabled(
                    has_saved && valid_name && rename_available,
                    egui::Button::new("Rename").min_size(egui::vec2(right, 28.0)),
                )
                .clicked()
            {
                if let Some(index) = self.selected_saved_selection {
                    self.saved_selections[index].0 = name.clone();
                    self.project_dirty = true;
                }
            }
        });

        if ui
            .add_enabled(has_saved, egui::Button::new("Delete Saved Selection"))
            .clicked()
        {
            if let Some(index) = self.selected_saved_selection {
                self.saved_selections.remove(index);
                if let Some(active) = self.active_saved_selection {
                    self.active_saved_selection = if active == index {
                        None
                    } else if active > index {
                        Some(active - 1)
                    } else {
                        Some(active)
                    };
                }
                self.selected_saved_selection = None;
                self.selection_name_input.clear();
                self.project_dirty = true;
            }
        }

        crate::interaction::small(ui,
            "Save New copies the working mask. \
             Double-click a name to load it. \
             Only the active selection is highlighted.",
        );

        ui.add_space(12.0);
        ui.separator();

        // ========================================
        // Brush / Erase
        // ========================================

        crate::interaction::label(ui,"Tool");

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selection_erase, false, "Brush");

            ui.selectable_value(&mut self.selection_erase, true, "Erase");
        });

        ui.add_space(12.0);

        // ========================================
        // Brush settings
        // ========================================

        crate::interaction::label(ui,"Brush Size");

        ui.add(numeric::Slider::new(&mut self.selection_size, 2.0..=500.0).suffix(" px"));

        ui.add_space(8.0);

        crate::interaction::label(ui,"Hardness");

        ui.add(numeric::Slider::new(&mut self.selection_hardness, 0.0..=1.0).show_value(true));

        ui.add_space(8.0);

        crate::interaction::label(ui,"Opacity");

        ui.add(numeric::Slider::new(&mut self.selection_opacity, 0.0..=1.0).show_value(true));

        ui.add_space(12.0);
        ui.separator();

        // ========================================
        // Mask visibility
        // ========================================

        ui.checkbox(&mut self.selection_show_mask, "Show Selection Mask");

        ui.add_space(10.0);

        // ========================================
        // Undo / Redo
        // ========================================

        let can_undo = !self.selection_undo.is_empty();
        let can_redo = !self.selection_redo.is_empty();

        ui.columns(2, |columns| {
            let left_width = columns[0].available_width();
            let right_width = columns[1].available_width();

            if columns[0]
                .add_enabled_ui(can_undo, |ui| {
                    ui.add_sized([left_width, 28.0], egui::Button::new("Undo"))
                })
                .inner
                .clicked()
            {
                self.selection_undo_action(&ctx);
            }

            if columns[1]
                .add_enabled_ui(can_redo, |ui| {
                    ui.add_sized([right_width, 28.0], egui::Button::new("Redo"))
                })
                .inner
                .clicked()
            {
                self.selection_redo_action(&ctx);
            }
        });

        ui.add_space(6.0);

        // ========================================
        // Clear / Invert
        // ========================================

        ui.columns(2, |columns| {
            let left_width = columns[0].available_width();
            let right_width = columns[1].available_width();

            if columns[0]
                .add_sized([left_width, 28.0], egui::Button::new("Clear Mask"))
                .clicked()
            {
                if self.selection_mask.is_some() {
                    self.selection_checkpoint();

                    if let Some(mask) = self.selection_mask.as_mut() {
                        mask.clear();
                    }
                    self.active_saved_selection = None;
                    self.selected_saved_selection = None;
                    self.selection_name_input.clear();

                    self.refresh_selection_overlay(&ctx);
                }
            }

            if columns[1]
                .add_sized([right_width, 28.0], egui::Button::new("Invert Mask"))
                .clicked()
            {
                if self.selection_mask.is_none() {
                    if let Ok(mask) = masking::SelectionMask::new(
                        self.image_size[0] as u32,
                        self.image_size[1] as u32,
                    ) {
                        self.selection_mask = Some(mask);
                    }
                }

                if self.selection_mask.is_some() {
                    self.selection_checkpoint();

                    if let Some(mask) = self.selection_mask.as_mut() {
                        mask.invert();
                    }

                    self.refresh_selection_overlay(&ctx);
                }
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // ========================================
        // Smart Selection
        // ========================================

        theme::section(ui, "Smart Selection");

        ui.add_enabled_ui(false, |ui| {
            ui.add_sized(
                [ui.available_width(), 30.0],
                egui::Button::new("AI Select Object (Coming Soon)"),
            );
        });

        ui.add_space(6.0);

        crate::interaction::small(ui,
            "Future AI tools will identify objects, \
             clothing, and people automatically.",
        );

        ui.add_space(14.0);
        ui.separator();

        crate::interaction::small(ui,
            "Left-drag: paint or erase\n\
             Space + left-drag / middle-drag: pan",
        );
    }
    fn show_viewer(&mut self, ui: &mut egui::Ui) {
        if self.texture.is_none() {
            ui.centered_and_justified(|ui| {
                ui.heading("Open an image to begin");
            });
            return;
        }

        if !self.generation_workspace {
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
            crate::interaction::label(ui,format!("{:.1}%", self.zoom * 100.0));
        });

        ui.horizontal(|ui| {
            if self.preview_rx.is_some() {
                ui.spinner();
                crate::interaction::small(ui,"Rendering adjusted preview...");
            }
            if let Some(texture) = &self.processed_texture {
                let size = texture.size();
                crate::interaction::small(ui,format!("Adjusted: {} x {} px", size[0], size[1]));
            } else {
                crate::interaction::small(ui,"Adjusted preview: pending");
            }
        });
        ui.separator();

        }
        let available = ui.available_size();

        if available.x <= 0.0 || available.y <= 0.0 {
            return;
        }

        if self.fit_pending {
            self.fit_image(available);
        }

        let (canvas, response) = ui.allocate_exact_size(available, Sense::hover());

        let painter = ui.painter().with_clip_rect(canvas);

        painter.rect_filled(canvas, 0.0, Color32::from_gray(20));

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
                i.pointer.button_double_clicked(fit_button) || i.pointer.button_double_clicked(egui::PointerButton::Primary),
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

        if (self.comparison_enabled || self.version_comparison) && primary_pressed && !self.selection_brush_enabled {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                if canvas.contains(pos)
                    && image_rect.contains(pos)
                    && (self.processed_texture.is_some() || self.version_comparison)
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

        let brush_takes_primary = self.selection_brush_enabled
            && !space_down
            && matches!(self.controls.pan, PanButton::Left | PanButton::SpaceLeft);

        if button_pressed
            && pointer_inside
            && modifier_ok
            && !self.divider_dragging
            && !brush_takes_primary
        {
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

        let (brush_down, brush_pressed) = ui.ctx().input(|i| {
            (
                i.pointer.button_down(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Primary),
            )
        });

        let brush_active =
            self.selection_brush_enabled && !space_down && !self.divider_dragging && !self.panning;

        if !brush_down || !brush_active {
            self.selection_last_point = None;
            self.selection_stroke_active = false;
        }

        if brush_active && brush_down {
            if let Some(pos) = pointer_position {
                if canvas.contains(pos) && image_rect.contains(pos) {
                    if self.selection_mask.is_none() {
                        if let Ok(mask) = masking::SelectionMask::new(
                            self.image_size[0] as u32,
                            self.image_size[1] as u32,
                        ) {
                            self.selection_mask = Some(mask);
                        }
                    }

                    let current = Pos2::new(
                        (pos.x - image_rect.left()) / self.zoom,
                        (pos.y - image_rect.top()) / self.zoom,
                    );

                    if !self.selection_stroke_active {
                        self.selection_checkpoint();
                        self.selection_stroke_active = true;
                        self.selection_last_point = None;
                    }

                    let changed = brush_pressed || self.selection_last_point != Some(current);

                    if changed {
                        let previous = if brush_pressed {
                            current
                        } else {
                            self.selection_last_point.unwrap_or(current)
                        };

                        let dx = current.x - previous.x;
                        let dy = current.y - previous.y;
                        let distance = (dx * dx + dy * dy).sqrt();

                        let radius = self.selection_size * 0.5;
                        let spacing = (radius * 0.3).max(1.0);

                        let steps = (distance / spacing).ceil().max(1.0).min(4096.0) as usize;

                        if let Some(mask) = self.selection_mask.as_mut() {
                            let mode = if self.selection_erase {
                                masking::BrushMode::Erase
                            } else {
                                masking::BrushMode::Paint
                            };

                            for step in 0..=steps {
                                let t = step as f32 / steps as f32;

                                mask.dab(
                                    previous.x + dx * t,
                                    previous.y + dy * t,
                                    radius,
                                    self.selection_hardness,
                                    self.selection_opacity,
                                    mode,
                                );
                            }
                        }

                        self.selection_last_point = Some(current);
                        self.refresh_selection_overlay(ui.ctx());
                        ui.ctx().request_repaint();
                    }
                } else {
                    self.selection_last_point = None;
                    self.selection_stroke_active = false;
                }
            } else {
                self.selection_last_point = None;
                self.selection_stroke_active = false;
            }
        }

        if self.version_comparison {
            if let (Some(left), Some(right)) = (&self.version_texture_a, &self.version_texture_b) {
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(right.id(), image_rect, uv, Color32::WHITE);
                let split = image_rect.left() + image_rect.width() * self.comparison_position;
                let clip = Rect::from_min_max(
                    canvas.min,
                    Pos2::new(split.clamp(canvas.left(), canvas.right()), canvas.bottom()),
                );
                painter.with_clip_rect(clip).image(left.id(), image_rect, uv, Color32::WHITE);
                if split >= canvas.left() && split <= canvas.right() && image_rect.intersects(canvas) {
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
        } else if let Some(original) = &self.texture {
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

            if self.selection_show_mask {
                if let Some(selection) = &self.selection_texture {
                    painter.image(
                        selection.id(),
                        image_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }

        if self.selection_brush_enabled {
            if let Some(pos) = pointer_position {
                if canvas.contains(pos) && image_rect.contains(pos) {
                    painter.circle_stroke(
                        pos,
                        (self.selection_size * self.zoom * 0.5).max(1.0),
                        egui::Stroke::new(1.5, Color32::WHITE),
                    );
                }
            }
        }

        let over_divider = (self.comparison_enabled || self.version_comparison)
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

        if !self.selection_brush_enabled && (self.divider_dragging || over_divider) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if self.panning {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        }
    }
}

impl eframe::App for PixelForge {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_project_jobs();
        for (_, project) in &self.inactive_tabs {
            project.cancel_project_jobs();
        }
        ai_generation::stop_managed_backend();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.prompt_guide.save_layout(storage);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        startup::ready(_frame);
        for action in self.interaction.begin_frame(ctx) {self.edit_command(ctx,action);}
        self.updater.poll(ctx,self.interaction.automatic_updates);
        self.display.update(ctx);
        self.display.resize_edges(ctx);
        // Keep native window persistence current even while the canvas is idle.
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        if ctx.input(|i|i.viewport().close_requested()) && self.dependency_manager.busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dependency_manager.open=true;
            self.dependency_manager.message="Finish or cancel dependency setup before closing PixelForge".into();
        }
        self.poll_open_projects(ctx);
        // Intercept the native window X button before the
        // operating system closes the application.
        if ctx.input(|i| i.viewport().close_requested()) {
            if (self.project_dirty || self.inactive_tabs.iter().any(|(_, tab)| tab.project_dirty))
                && !self.exit_allowed
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.exit_confirmation = true;
            }
        }

        self.dependency_manager.poll(ctx,self.install_folder.clone(),self.generation_checkpoints.clone());
        self.ensure_workspace(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            let bar=ui.available_rect_before_wrap();
            let title=ui.painter().layout_no_wrap("Pixel Forge".into(),egui::TextStyle::Button.resolve(ui.style()),ui.visuals().text_color());
            let title_pos=egui::pos2(ctx.screen_rect().center().x-title.size().x/2.0,bar.top()+(ui.spacing().interact_size.y-title.size().y)/2.0);
            let left_width=(title_pos.x-bar.left()-12.0).max(100.0);
            let mut menu_content_end = bar.left();
            ui.horizontal(|ui| {
            ui.set_min_height(28.0);
            ui.allocate_ui_with_layout(egui::vec2(left_width,ui.spacing().interact_size.y),egui::Layout::left_to_right(egui::Align::Center),|ui| {
            egui::ScrollArea::horizontal().id_salt("top_bar_menus").max_width(left_width)
                .scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() })
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui,|ui| {
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

                    let can_save = (self.generation_workspace || self.original_pixels.is_some()) && !self.exporting;

                    if ui
                        .add_enabled(can_save, egui::Button::new("Save Project"))
                        .clicked()
                    {
                        ui.close();
                        if self.generation_workspace {self.save_generation_project(false);} else {self.save_project(false);}
                    }

                    if ui
                        .add_enabled(can_save, egui::Button::new("Save Project As..."))
                        .clicked()
                    {
                        ui.close();
                        if self.generation_workspace {self.save_generation_project(true);} else {self.save_project(true);}
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
                    interaction::keep_selection(ctx);
                    for action in [interaction::Action::Undo,interaction::Action::Redo,interaction::Action::Copy,interaction::Action::Cut,interaction::Action::Paste] {
                        if action==interaction::Action::Copy {ui.separator();}
                        let enabled=self.can_edit_command(ctx,action);
                        if ui.add_enabled(enabled,egui::Button::new(action.label()).shortcut_text(self.interaction.shortcut(action))).clicked() {
                            ui.close();self.edit_command(ctx,action);
                        }
                    }
                });

                ui.add_enabled_ui(self.generation_workspace, |ui| {ui.menu_button("Import", |ui| {if ui.button("Import Images...").clicked() {ui.close();self.import_generation_images(ctx);}});});
                ui.menu_button("Export", |ui| {
                    let can_export = (if self.generation_workspace {!self.generation_gallery.is_empty()} else {self.original_pixels.is_some()}) && !self.exporting;

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as PNG..."))
                        .clicked()
                    {
                        ui.close();
                        if self.generation_workspace {self.export_generation_selected(image::ImageFormat::Png,"png");} else {self.start_export(ExportFormat::Png);}
                    }

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as JPEG..."))
                        .clicked()
                    {
                        ui.close();
                        if self.generation_workspace {self.export_generation_selected(image::ImageFormat::Jpeg,"jpg");} else {self.start_export(ExportFormat::Jpeg);}
                    }

                    if ui
                        .add_enabled(can_export, egui::Button::new("Export as WebP..."))
                        .clicked()
                    {
                        ui.close();
                        if self.generation_workspace {self.export_generation_selected(image::ImageFormat::WebP,"webp");} else {self.start_export(ExportFormat::WebP);}
                    }
                });

                if ui.button("Settings").clicked() {self.show_settings=!self.show_settings;}
                let missing=self.dependency_manager.missing();
                let label = if missing > 0 {
                    egui::RichText::new(format!("Setup ({missing})")).color(egui::Color32::YELLOW).strong()
                } else {
                    egui::RichText::new("Setup")
                };
                ui.menu_button(label, |ui| {
                    if ui.button("Setup & Diagnostics").clicked() {
                        self.dependency_manager.diagnostics_open = true;
                        ui.close();
                    }
                    if ui.button("Dependencies").clicked() {
                        self.dependency_manager.open = true;
                        ui.close();
                    }
                    if ui.button("Model Library").clicked() {
                        self.model_library_open = true;
                        ui.close();
                    }
                });
                let updates=if self.updater.available() {egui::RichText::new("Updates").color(egui::Color32::YELLOW).strong()}else{egui::RichText::new("Updates")};
                if ui.button(updates).clicked() {self.updater.open=!self.updater.open;}
                ui.separator();

                if self.project_dirty {
                    ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                }
                menu_content_end = ui.next_widget_position().x.min(bar.left()+left_width);
            });
            });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui|self.display.window_controls(ui));
            });
            let title_pos=egui::pos2(title_pos.x, bar.top()+(28.0-title.size().y)/2.0);
            let title_right=title_pos.x+title.size().x;
            let controls_left=bar.right()-112.0;
            // Register this explicit target after the menu's child UI, using the
            // actual last menu item rather than its much wider allocated space.
            if menu_content_end < controls_left {
                self.display.title_drag(ui,egui::Rect::from_min_max(egui::pos2(menu_content_end,bar.top()+5.0),egui::pos2(controls_left,bar.top()+28.0)));
            }
            ui.painter().galley(title_pos,title.clone(),ui.visuals().strong_text_color());
            ui.painter().galley(title_pos+egui::vec2(0.8,0.0),title.clone(),ui.visuals().strong_text_color());
            ui.painter().galley(title_pos+egui::vec2(0.0,0.25),title,ui.visuals().strong_text_color());
            let version=format!("{}v{}",if profile::is_dev(){"Dev "}else{""},env!("CARGO_PKG_VERSION"));
            ui.painter().with_clip_rect(egui::Rect::from_min_max(egui::pos2(title_right+8.0,bar.top()),egui::pos2(controls_left,bar.top()+28.0))).text(egui::pos2(title_right+8.0,title_pos.y),egui::Align2::LEFT_TOP,version,egui::TextStyle::Small.resolve(ui.style()),ui.visuals().weak_text_color());
        });
        self.show_project_tabs(ctx);
        egui::SidePanel::right(egui::Id::new(("tools", self.active_tab)))
            .exact_width(360.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Sidebar sections and model controls are larger than canvas/status text.
                for text in [egui::TextStyle::Body,egui::TextStyle::Button,egui::TextStyle::Small,egui::TextStyle::Monospace] {
                    if let Some(font)=ui.style_mut().text_styles.get_mut(&text) {font.size=16.0;}
                }
                if let Some(font)=ui.style_mut().text_styles.get_mut(&egui::TextStyle::Heading) {font.size=21.0;}
                ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().icon_width=22.0;
                ui.spacing_mut().icon_width_inner=16.0;
                ui.spacing_mut().combo_width=160.0;
                ui.spacing_mut().slider_width=(ui.available_width()-150.0).max(60.0);
                ui.horizontal_wrapped(|ui| {
                    // Keep all four tool tabs compact at the existing sidebar width.
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 5.0);
                    let edit_clicked = ui
                        .selectable_label(!self.ai_tools_tab && !self.selection_tools_tab && !self.generation_tools_tab, "Edit")
                        .clicked();

                    let ai_clicked = ui
                        .selectable_label(
                            self.ai_tools_tab && !self.selection_tools_tab && !self.generation_tools_tab,
                            "AI Tools",
                        )
                        .clicked();

                    let generation_clicked = ui.selectable_label(self.generation_tools_tab, "Generate").clicked();

                    let selection_clicked = ui
                        .selectable_label(self.selection_tools_tab, "Selection")
                        .clicked();

                    if edit_clicked || ai_clicked || selection_clicked || generation_clicked {
                        self.selection_tools_tab = selection_clicked;
                        self.ai_tools_tab = ai_clicked;
                        self.generation_tools_tab = generation_clicked;
                        if generation_clicked {
                            self.generation_workspace = true;
                            if self.generation_models_rx.is_none() && self.generation_checkpoints.is_empty() {
                                self.refresh_generation_models(ctx);
                            }
                        }


                        // Enable painting only in Selection tab.
                        // Do not clear the existing selection.
                        self.selection_brush_enabled = selection_clicked;
                        self.selection_last_point = None;
                        self.selection_stroke_active = false;
                        self.panning = false;

                        if let Some(folder) = profile::settings_root() {

                            if std::fs::create_dir_all(&folder).is_ok() {
                                let selected = if selection_clicked {
                                    "selection"
                                } else if ai_clicked {
                                    "ai"
                                } else if generation_clicked {
                                    "generation"
                                } else {
                                    "edit"
                                };

                                let _ = std::fs::write(folder.join("sidebar_tab.txt"), selected);
                            }
                        }
                    }
                });
                ui.separator();

                if !self.generation_workspace {
                // Image information remains visible in both tabs.
                ui.heading("Image Information");

                if let Some(path) = &self.image_path {
                    if let Some(name) = path.file_name() {
                        crate::interaction::label(ui,name.to_string_lossy());
                    }

                    crate::interaction::label(ui,format!(
                        "{} x {} pixels",
                        self.image_size[0], self.image_size[1]
                    ));

                    crate::interaction::label(ui,format!("{:.2} MB", self.file_size as f64 / 1_048_576.0));
                } else {
                    crate::interaction::label(ui,"No image loaded");
                }

                ui.separator();

                }
                // A solid, permanently allocated gutter never overlays controls on hover.
                ui.spacing_mut().scroll.floating = false;
                ui.spacing_mut().scroll.bar_width = 10.0;
                ui.spacing_mut().scroll.bar_inner_margin = 6.0;
                ui.spacing_mut().scroll.bar_outer_margin = 0.0;
                let sidebar_right = ui.max_rect().right();
                let sidebar_scroll = egui::ScrollArea::vertical()
                    .id_salt("pixelforge_tools_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.generation_tools_tab {
                            self.show_generation_sidebar(ui);
                        } else if self.selection_tools_tab {
                            self.show_selection_sidebar(ui);
                        } else if !self.ai_tools_tab {
                            if self.texture.is_some() {
                                self.show_edit_sidebar(ui,ctx);
                            }

                        } else {
                            self.show_photo_restoration(ui, ctx);
                            ui.horizontal(|ui| {
                                                            crate::interaction::label(ui,"AI Result Strength");

                                                            ui.with_layout(
                                                                egui::Layout::right_to_left(egui::Align::Center),
                                                                |ui| {
                                                                    crate::interaction::label(ui,format!("{:.1}%", self.ai_strength * 100.0));
                                                                },
                                                            );
                                                        });

                                                        // egui Slider has its own internal width.
                                                        // Set that width explicitly rather than
                                                        // just enlarging its allocated rectangle.
                                                        let old_slider_width = ui.spacing().slider_width;

                                                        ui.spacing_mut().slider_width = (ui.available_width() - 20.0).max(80.0);

                                                        if ui
                                                            .add_enabled(!self.ai_running,
                                                                numeric::Slider::new(&mut self.ai_strength, 0.0..=1.0)
                                                                    .step_by(0.05)
                                                                    .show_value(false),
                                                            )
                                                            .changed()
                                                        {
                                                            self.refresh_ai_preview(ctx);
                                                        }

                                                        ui.spacing_mut().slider_width = old_slider_width;

                                                        crate::interaction::label(ui,"Controls the blend for the selected AI result (Upscale, Denoise, Sharpen, or Deblur). 0% = baseline; 100% = full AI result.");
                            ui.separator();
                            theme::section(ui, "AI Upscaling");
                            ui.separator();
                            crate::interaction::label(ui,"Real-ESRGAN");
                            if self.ai_gpus.is_empty() && self.ai_gpu_error.is_none() { self.refresh_vulkan_gpus(); }
                            ui.add_enabled_ui(!self.ai_running, |ui| {
                                crate::interaction::label(ui,"Upscaling model");
                                egui::ComboBox::from_id_salt("sidebar_Upscaling_model").width(ui.available_width()).truncate()
                                    .selected_text(["Photo", "Illustration / Anime"][self.ai_model.min(1)])
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.ai_model, 0, "Photo");
                                        ui.selectable_value(&mut self.ai_model, 1, "Illustration / Anime");
                                    });
                                let gpu_name = self.ai_gpus.iter().find(|gpu| gpu.id == self.ai_gpu_id)
                                    .map(|gpu| format!("{} (GPU {})", gpu.name, gpu.id))
                                    .unwrap_or_else(|| {
                                        ai_upscale::preferred_gpu_id(&self.ai_gpus)
                                            .and_then(|id| self.ai_gpus.iter().find(|gpu| gpu.id == id))
                                            .map(|gpu| format!("Automatic — {}", gpu.name))
                                            .unwrap_or_else(|| "Automatic (Recommended)".to_owned())
                                    });
                                crate::interaction::label(ui,"Vulkan GPU");
                                egui::ComboBox::from_id_salt("sidebar_Vulkan_GPU").width(ui.available_width()).truncate().selected_text(gpu_name)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.ai_gpu_id, -1, "Automatic (Recommended)");
                                        for gpu in &self.ai_gpus {
                                            ui.selectable_value(&mut self.ai_gpu_id, gpu.id, format!("{} (GPU {})", gpu.name, gpu.id));
                                        }
                                    });
                                if self.ai_gpus.is_empty() {
                                    crate::interaction::label(ui,self.ai_gpu_error.as_deref().unwrap_or("No Vulkan GPUs detected"));
                                }
                                if ui.small_button("Refresh GPU list").clicked() { self.refresh_vulkan_gpus(); }
                                crate::interaction::label(ui,"Tile size");
                                egui::ComboBox::from_id_salt("sidebar_Tile_size").width(ui.available_width()).truncate()
                                    .selected_text(if self.ai_tile_size == 0 { "Auto".to_owned() } else { self.ai_tile_size.to_string() })
                                    .show_ui(ui, |ui| {
                                        for tile in [0, 32, 64, 128, 256, 512] {
                                            ui.selectable_value(&mut self.ai_tile_size, tile, if tile == 0 { "Auto".to_owned() } else { tile.to_string() });
                                        }
                                    });
                            });

                            ui.add_space(8.0);

                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                let enabled = !self.ai_running
                                    && !self.ai_saving
                                    && !self.exporting
                                    && self.original_pixels.is_some();

                                let spacing = ui.spacing().item_spacing.x;
                                let width = ((ui.available_width() - spacing) / 2.0).max(60.0);

                                if ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::new("Upscale 2x")
                                            .min_size(egui::vec2(width, 36.0)),
                                    )
                                    .clicked()
                                {
                                    self.start_ai_upscale(ctx, 2);
                                }

                                if ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::new("Upscale 4x")
                                            .min_size(egui::vec2(width, 36.0)),
                                    )
                                    .clicked()
                                {
                                    self.start_ai_upscale(ctx, 4);
                                }
                            });
                            if self.ai_running {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    crate::interaction::label(ui,if let Some(operation) = self.ai_restoration { if operation == ai_restoration::Operation::Sharpen { "AI sharpening running..." } else { "NAFNet deblurring running..." } } else if self.ai_is_denoise { "AI denoising running..." } else { "GPU inference running..." });
                                    if ui.button(if self.ai_restoration.is_some() { "Cancel Restoration" } else if self.ai_is_denoise { "Cancel Denoise" } else { "Cancel Upscale" }).clicked() {
                                        self.ai_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                                        self.ai_status = if self.ai_restoration.is_some() { "Cancelling AI restoration..." } else if self.ai_is_denoise { "Cancelling AI denoise..." } else { "Cancelling AI upscale..." }.into();
                                    }
                                });
                            }

                            if !self.ai_status.is_empty() {
                                crate::interaction::label(ui,&self.ai_status);
                            }

                            if self.ai_result_texture.is_some() {
                                if self.generation_workspace {
                                    crate::interaction::small(ui,"AI results join the thumbnail history automatically. After changing strength, keep a new blend version.");
                                    if ui.add_enabled(!self.ai_running,egui::Button::new("Keep AI Blend as Version")).clicked() {
                                        if let Err(error)=self.append_ai_workspace_result(ctx,"AI blend") {self.ai_status=error;}
                                    }
                                } else if ui.button("Show AI Result").clicked() {
                                    self.ai_show_result = true;
                                }

                                if ui
                                    .add_enabled(
                                        !self.ai_saving,
                                        egui::Button::new("Save Result..."),
                                    )
                                    .clicked()
                                {
                                    self.save_ai_result(ctx);
                                }

                                if ui.button("Open Output Folder").clicked() {
                                    if let Some(path) = &self.ai_result_path {
                                        let _ = std::process::Command::new("explorer.exe")
                                            .arg("/select,")
                                            .arg(path)
                                            .spawn();
                                    }
                                }
                            }

                            ui.add_space(12.0);
                            ui.separator();


                        }
                    });
                if sidebar_scroll.content_size.y <= sidebar_scroll.inner_rect.height() {
                    // Keep the empty track, but hide the full-length handle when it cannot scroll.
                    let track = egui::Rect::from_min_max(
                        egui::pos2(sidebar_right-10.0, sidebar_scroll.inner_rect.top()),
                        egui::pos2(sidebar_right, sidebar_scroll.inner_rect.bottom()));
                    ui.painter().rect_filled(track, 5, ui.visuals().extreme_bg_color);
                }
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if self.generation_workspace {
                ui.set_min_height(60.0);
                self.show_generation_progress(ui);
                self.show_workspace_details(ui);
                if self.ai_running {ui.spinner();crate::interaction::label(ui,&self.ai_status);}
                if self.exporting || self.ai_saving {crate::interaction::label(ui,&self.export_stage);}
                if !self.status.is_empty() && self.status!="Ready" {crate::interaction::label(ui,&self.status);}
                return;
            }
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 76.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(76.0);
                    if self.exporting || self.export_result.is_some() {
                        ui.horizontal(|ui| {
                            if self.exporting && self.export_encoding {
                                ui.spinner();
                                crate::interaction::label(ui,&self.export_stage);
                            } else if self.export_result == Some(false) {
                                ui.colored_label(egui::Color32::LIGHT_RED, "Export failed");
                            } else {
                                crate::interaction::label(ui,&self.export_stage);
                                crate::interaction::label(ui,format!("{:.0}%", self.export_progress * 100.0));
                            }
                        });

                        if self.export_result != Some(false) {
                            ui.add(
                                egui::ProgressBar::new(self.export_progress)
                                    .desired_width(ui.available_width()),
                            );
                        }

                        if self.export_result == Some(true) {
                            ui.horizontal(|ui| {
                                crate::interaction::label(ui,"Image saved successfully");

                                if ui.button("Open Image Folder").clicked() {
                                    if let Some(path) = &self.last_export_path {
                                        let result = std::process::Command::new("explorer.exe")
                                            .arg("/select,")
                                            .arg(path)
                                            .spawn();

                                        if let Err(err) = result {
                                            self.status =
                                                format!("Could not open image folder: {err}");
                                        }
                                    }
                                }
                            });
                        }
                    }
                },
            );
            ui.horizontal(|ui| {
                crate::interaction::label(ui,&self.status);

                if self.texture.is_some() {
                    ui.separator();

                    crate::interaction::label(ui,format!("{} x {}", self.image_size[0], self.image_size[1]));

                    ui.separator();

                    crate::interaction::label(ui,format!("Zoom: {:.1}%", self.zoom * 100.0));
                }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).fill(theme::CANVAS)).show(ctx, |ui| {
            if self.generation_workspace {
                self.show_generation_workspace(ui);
            } else if self.ai_show_result {
                self.show_ai_result(ctx, ui);
            } else {
                self.show_viewer(ui);
            }
        });

        let workspace_busy = self.install_rx.is_some()
            || self.backend_jobs_running()
            || self
                .inactive_tabs
                .iter()
                .any(|(_, project)| project.backend_jobs_running());
        if self.dependency_manager.show_diagnostics(
            ctx,
            workspace_busy,
            &self.generation_models_status,
            self.generation_models_rx.is_none(),
        ) {
            self.refresh_generation_models(ctx);
        }
        if self.dependency_manager.show(
            ctx,
            self.install_folder.clone(),
            self.generation_checkpoints.clone(),
            workspace_busy,
            |ui| self.prompt_guide.show_ollama_dependency(ui, &self.generation_checkpoint),
        ) {
            self.model_library_open = true;
        }
        self.handle_file_drops(ctx);
        self.show_model_library(ctx);
        self.show_model_removal(ctx);
        self.show_controls_settings(ctx);
        self.show_unsaved_prompt(ctx);
        self.show_exit_confirmation(ctx);
        let can_restart = !self.project_dirty && !self.generation_running && !self.ai_running
            && !self.exporting && !self.ai_saving && self.install_rx.is_none() && !self.dependency_manager.busy()
            && !self.inactive_tabs.iter().any(|(_,tab)|tab.project_dirty || tab.generation_running || tab.ai_running
                || tab.exporting || tab.ai_saving || tab.install_rx.is_some());
        if self.updater.show(ctx, can_restart, &mut self.interaction) {
            self.exit_allowed=true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result {
    let Some(_startup) = startup::begin() else { return Ok(()); };
    let icon_image = image::load_from_memory(include_bytes!("../assets/pixelforge_window.png"))
        .expect("Failed to load PixelForge window icon")
        .into_rgba8();

    let icon_data = egui::IconData {
        rgba: icon_image.as_raw().clone(),
        width: icon_image.width(),
        height: icon_image.height(),
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(profile::window_title())
        .with_decorations(false)
        .with_drag_and_drop(true)
        .with_icon(icon_data)
        .with_min_inner_size([800.0, 500.0]);
    viewport=viewport.with_inner_size([1400.0,900.0]);
    let options = eframe::NativeOptions {
        viewport,
        persist_window: true,
        persistence_path: profile::settings_root().map(|path|path.join("window-state.ron")),
        ..Default::default()
    };

    eframe::run_native(
        "PixelForge",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            // A single readable default for all non-heading text, including small labels.
            cc.egui_ctx.style_mut(|style| {
                style.spacing.icon_width=22.0;
                style.spacing.icon_width_inner=16.0;
                style.spacing.interact_size.y=26.0;
                for text in [egui::TextStyle::Body,egui::TextStyle::Button,egui::TextStyle::Small,egui::TextStyle::Monospace] {
                    if let Some(font)=style.text_styles.get_mut(&text) {font.size=16.0;}
                }
            });

            let controls = Controls::load();
            let native_scale = cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0);

            cc.egui_ctx
                .set_pixels_per_point(native_scale * controls.ui_scale);
            let mut app = PixelForge::default();
            if !app.install_folder_manual {
                if let Some(folder) = model_installer::detect_models_folder(&[]) {
                    app.install_folder = Some(folder);
                }
            }
            if let Some(storage) = cc.storage {
                app.prompt_guide.restore_layout(storage);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod conversation_tests {
    use super::*;
    #[test]
    fn cyberrealistic_xl_and_pony_use_their_own_generation_defaults() {
        let (steps,cfg,_) = PixelForge::recommended_generation_settings("CyberRealisticXLPlay_V10.0_FP16.safetensors");
        assert_eq!((steps,cfg),(30,4.0));
        let (steps,cfg,_) = PixelForge::recommended_generation_settings("CyberRealisticPony_V18.0_F16.safetensors");
        assert_eq!((steps,cfg),(30,5.0));
    }
    #[test]
    fn photo_presets_favor_texture_and_keep_cleanup_strength_separate() {
        assert_eq!(restoration_preset_settings(0), (1, 0.65));
        assert_eq!(restoration_preset_settings(1), (2, 0.80));
        assert_eq!(restoration_preset_settings(2), (0, 0.75));
        let mut app = PixelForge::default();
        app.ai_strength = 0.2;
        assert_eq!(app.restoration_strength, 0.65);
    }
    #[test]
    fn edit_model_selection_does_not_change_generation_model() {
        assert_eq!(editing_checkpoint("base.safetensors", "edit.safetensors", true), "edit.safetensors");
        assert_eq!(editing_checkpoint("base.safetensors", "edit.safetensors", false), "base.safetensors");
        assert_eq!(editing_checkpoint("base.safetensors", "", true), "base.safetensors");
    }
    #[test]
    fn sequential_context_and_metadata_roundtrip() {
        let mut s = settings_from_json(&serde_json::json!({"prompt":"A red car", "refine_prompt":"A blue car", "refine_parent":0}));
        assert_eq!(conversation_context(&s), "A red car, A blue car");
        s.prompt = conversation_context(&s);
        s.refine_prompt = Some("Dramatic sunset".into());
        s.refine_parent = Some(1);
        let restored = settings_from_json(&settings_json(&s));
        assert_eq!(conversation_context(&restored), "A red car, A blue car, Dramatic sunset");
        assert_eq!(restored.refine_parent, Some(1));
    }
    #[test]
    fn imported_image_can_begin_conversation() {
        let s = settings_from_json(&serde_json::json!({"refine_prompt":"A blue car"}));
        assert_eq!(conversation_context(&s), "A blue car");
    }
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    fn drop_test_files(app:&mut PixelForge,ctx:&egui::Context,paths:&[PathBuf]) {
        let mut input=egui::RawInput::default();
        input.dropped_files=paths.iter().map(|path|egui::DroppedFile {path:Some(path.clone()),..Default::default()}).collect();
        let _=ctx.run(input,|ctx|app.handle_file_drops(ctx));
    }
    #[test]
    fn crop_creates_a_version_and_preserves_edits_masks_and_original_on_save() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        let pixels=image::RgbaImage::from_fn(8,6,|x,y|image::Rgba([x as u8*20,y as u8*20,40,255]));
        let mask=masking::SelectionMask::from_pixels(8,6,(0..48).map(|n|if n%2==0 {255}else{0}).collect()).unwrap();
        let mut editor=studio::EditorState::default();
        editor.global.exposure=0.5;editor.local.brightness=10.0;
        editor.mask=Some(mask.clone());editor.saved_selections.push(("Subject".into(),mask));
        let rendered=editor.render(&pixels);
        let texture=ctx.load_texture("crop_original",ColorImage::from_rgba_unmultiplied([8,6],rendered.as_raw()),TextureOptions::LINEAR);
        app.generation_gallery.push(GeneratedItem {group:None, editor,settings:settings_from_json(&serde_json::json!({"prompt":"Original","dimension_step":16,"resolution":4})),pixels:pixels.clone(),texture,seed:42});
        app.generation_workspace=true;app.bind_workspace_editor(&ctx);
        let mut crop=crop::Crop::new(0,[8,6]);
        crop.rect=Rect::from_min_max(egui::pos2(1.0,1.0),egui::pos2(6.0,5.0));
        crop.set_pixel_size(5,4,true,[8,6]);
        app.crop=Some(crop);app.apply_workspace_crop(&ctx);
        assert_eq!(app.generation_gallery.len(),2);
        assert_eq!(app.generation_gallery[0].pixels,pixels);
        let cropped=&app.generation_gallery[1];
        assert_eq!(cropped.pixels.dimensions(),(5,4));
        assert_eq!(cropped.pixels.get_pixel(0,0),pixels.get_pixel(1,1));
        assert_eq!(cropped.editor.global.exposure,0.5);
        assert_eq!(cropped.editor.mask.as_ref().unwrap().dimensions(),(5,4));
        assert_eq!(cropped.editor.mask.as_ref().unwrap().pixels()[0],0);
        assert_eq!(cropped.editor.saved_selections[0].1.dimensions(),(5,4));
        assert_eq!(cropped.settings.refine_parent,Some(0));
        assert_eq!(cropped.settings.dimension_step,16);
        let path=std::env::temp_dir().join(format!("pixelforge-crop-roundtrip-{}.pxf",std::process::id()));
        app.generation_project_path=Some(path.clone());app.save_generation_project(false);
        let mut loaded=PixelForge::default();loaded.open_generation_project(&path,&ctx);loaded.ensure_workspace(&ctx);
        assert_eq!(loaded.generation_gallery.len(),2);
        assert_eq!(loaded.original_pixels.as_ref().unwrap().dimensions(),(5,4));
        assert_eq!(loaded.selection_mask.as_ref().unwrap().dimensions(),(5,4));
        assert_eq!(loaded.generation_gallery[1].settings.dimension_step,16);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn old_resolution_settings_retain_ids_and_64_pixel_rounding() {
        let old=settings_from_json(&serde_json::json!({"resolution":1,"width":1344,"height":768}));
        assert_eq!(old.resolution,1);assert_eq!(old.dimension_step,64);
        assert_eq!(generation_dimensions(5,old.resolution,old.dimension_step),(1344,768));
        let fresh=settings_from_json(&serde_json::json!({"resolution":4,"dimension_step":32}));
        let restored=settings_from_json(&settings_json(&fresh));
        assert_eq!(restored.resolution,4);assert_eq!(restored.dimension_step,32);
    }
    #[test]
    fn dropped_images_and_editor_projects_open_without_losing_existing_work() {
        let root=std::env::temp_dir().join(format!("pixelforge-drops-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let first=root.join("first.PNG");let second=root.join("second.png");let bad=root.join("bad.png");
        image::RgbaImage::from_pixel(2,1,image::Rgba([40,40,40,255])).save(&first).unwrap();
        image::RgbaImage::from_pixel(2,1,image::Rgba([80,80,80,255])).save(&second).unwrap();
        std::fs::write(&bad,b"not an image").unwrap();
        let ctx=egui::Context::default();let mut app=PixelForge::default();
        drop_test_files(&mut app,&ctx,&[first,second]);
        assert_eq!(app.generation_gallery.len(),1);
        assert_eq!(app.inactive_tabs.len(),1);
        assert_eq!(app.inactive_tabs[0].1.generation_gallery.len(), 1);
        assert!(app.inactive_tabs[0].1.project_dirty);
        let active=app.active_tab;
        drop_test_files(&mut app,&ctx,&[bad]);
        assert_eq!(app.active_tab,active);
        assert_eq!(app.generation_gallery.len(),1);
        assert!(app.status.starts_with("Open failed:"));
        let project=root.join("editor.PXF");
        app.generation_workspace=false;app.project_path=Some(project.clone());app.save_project(false);
        assert!(project.is_file());
        drop_test_files(&mut app,&ctx,&[project]);
        assert_eq!(app.inactive_tabs.len(),2);
        assert_eq!(app.original_pixels.as_ref().unwrap().get_pixel(0,0)[0],80);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn dropped_studio_project_opens_beside_unsaved_project() {
        let path=std::env::temp_dir().join(format!("pixelforge-studio-drop-{}.pxf",std::process::id()));
        let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([100,100,100,255]));
        generation_project::save(&path,&generation_project::GalleryProject {
            items:vec![generation_project::GalleryImage {settings:serde_json::json!({"prompt":"Dropped project"}),pixels,seed:42}],
            selected:0,compare_index:0,compare:false,current:serde_json::json!({})
        }).unwrap();
        let ctx=egui::Context::default();let mut app=PixelForge::default();
        app.project_dirty=true;
        drop_test_files(&mut app,&ctx,&[path.clone()]);
        assert!(app.file_drops.is_empty());
        assert_eq!(app.inactive_tabs.len(), 1);
        assert!(app.inactive_tabs[0].1.project_dirty);
        assert!(app.inactive_tabs[0].1.generation_gallery.is_empty());
        assert!(!app.project_dirty);
        assert_eq!(app.generation_project_path,Some(path.clone()));
        assert_eq!(app.generation_gallery[0].seed,42);
        assert_eq!(app.generation_gallery[0].settings.prompt,"Dropped project");
        assert_eq!(app.original_pixels.as_ref().unwrap().get_pixel(0,0)[0],100);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn restoration_panel_renders_with_independent_prompt_and_model() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        app.generation_prompt = "A landscape".into();
        app.conversation_draft = "Change the sky".into();
        app.edit_checkpoint = ai_generation::QWEN_EDIT.into();
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(320.0,1200.0))), ..Default::default() };
        let output = ctx.run(input, |ctx| {egui::CentralPanel::default().show(ctx, |ui| app.show_photo_restoration(ui, ctx));});
        assert!(!output.shapes.is_empty());
        assert_eq!(app.restoration_model, ai_generation::FLUX_KLEIN);
        assert_eq!(app.restoration_steps, 4);
        assert_eq!(app.generation_prompt, "A landscape");
        assert_eq!(app.conversation_draft, "Change the sky");
        assert_eq!(app.edit_checkpoint, ai_generation::QWEN_EDIT);
    }
    #[test]
    fn rotation_updates_in_place_and_subsequent_versions_keep_orientation() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        app.generation_workspace = true;
        let pixels = image::RgbaImage::from_fn(3, 2, |x,y| image::Rgba([(x+y*3) as u8,20,30,127]));
        let texture = ctx.load_texture("rotate_test",ColorImage::from_rgba_unmultiplied([3,2],pixels.as_raw()),TextureOptions::LINEAR);
        app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels:pixels.clone(),texture,seed:12 });
        app.ensure_workspace(&ctx);
        app.selection_mask = Some(masking::SelectionMask::from_pixels(3,2,vec![255,0,0,0,0,0]).unwrap());
        app.rotate_workspace_version(&ctx, true);
        assert_eq!(app.generation_gallery.len(), 1);
        assert_eq!(app.generation_gallery[0].pixels.dimensions(), (2,3));
        assert_eq!(app.generation_gallery[0].pixels.get_pixel(1,0), pixels.get_pixel(0,0));
        assert_eq!(app.selection_mask.as_ref().unwrap().get(1,0), Some(255));
        app.keep_workspace_version(&ctx, "Manual edits");
        assert_eq!(app.generation_gallery[1].pixels.dimensions(), (2,3));
        app.generation_selected = 0;
        for _ in 0..3 { app.rotate_workspace_version(&ctx, true); }
        assert_eq!(app.generation_gallery[0].pixels, pixels);
        assert_eq!(app.generation_selected, 0);
        assert_eq!(app.generation_gallery.len(), 2);
        assert_eq!(app.generation_compare_index, 0);
    }
    #[test]
    fn tools_share_selection_and_thumbnail_switching_retains_adjustments_and_mask() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        app.generation_workspace=true;
        for n in 0..2 {
            let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([40+n,40,40,255]));
            let texture=ctx.load_texture(format!("test_{n}"),ColorImage::from_rgba_unmultiplied([2,1],pixels.as_raw()),TextureOptions::LINEAR);
            app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels,texture,seed:0});
        }
        app.ensure_workspace(&ctx);
        app.exposure=1.0;
        app.selection_adjustments.exposure=0.25;
        app.selection_adjustment_target=true;
        app.selection_mask=Some(masking::SelectionMask::from_pixels(2,1,vec![255,0]).unwrap());
        app.selection_tools_tab=true;
        app.ensure_workspace(&ctx);
        assert_eq!(app.workspace_editor,Some(0));
        assert_eq!(app.exposure,1.0);
        assert_eq!(app.selection_mask.as_ref().unwrap().pixels(),&[255,0]);
        app.ai_tools_tab=true;app.selection_tools_tab=false;
        app.ensure_workspace(&ctx);
        assert_eq!(app.exposure,1.0);
        app.generation_selected=1;app.ensure_workspace(&ctx);
        assert_eq!(app.exposure,0.0);
        app.generation_selected=0;app.ensure_workspace(&ctx);
        assert_eq!(app.exposure,1.0);
        assert_eq!(app.selection_mask.as_ref().unwrap().pixels(),&[255,0]);
        assert_eq!(app.selection_adjustments.exposure,0.25);
        assert_eq!(app.generation_gallery[0].pixels.get_pixel(0,0)[0],40);
        assert!(app.generation_gallery[0].editor.render(&app.generation_gallery[0].pixels).get_pixel(0,0)[0]>40);
    }
    #[test]
    fn unchecked_compare_hides_legacy_dividers_and_checked_compare_shows_one() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        for value in [40,80] {
            let pixels=image::RgbaImage::from_pixel(32,32,image::Rgba([value,value,value,255]));
            let texture=ctx.load_texture(format!("compare_visibility_{value}"),ColorImage::from_rgba_unmultiplied([32,32],pixels.as_raw()),TextureOptions::LINEAR);
            app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels,texture,seed:0});
        }
        app.generation_workspace=true;
        app.bind_workspace_editor(&ctx);
        app.comparison_enabled=true;
        app.version_comparison=true;
        app.divider_dragging=true;
        let frame=|app:&mut PixelForge| {
            let input=egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1400.0,900.0))),..Default::default()};
            ctx.run(input,|ctx| {egui::CentralPanel::default().show(ctx,|ui|app.show_generation_workspace(ui));})
        };
        let has_divider=|output:&egui::FullOutput|output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::LineSegment {points,stroke}=>points[0].x==points[1].x && (points[0].y-points[1].y).abs()>100.0 && stroke.width==2.0 && stroke.color==egui::Color32::WHITE,
            _=>false,
        });
        assert!(!has_divider(&frame(&mut app)));
        assert!(!app.comparison_enabled && !app.version_comparison && !app.divider_dragging);
        app.generation_compare=true;
        assert!(has_divider(&frame(&mut app)));
        app.generation_compare=false;
        assert!(!has_divider(&frame(&mut app)));
    }
    #[test]
    fn shared_compare_divider_drags_and_camera_survives_version_and_tool_changes() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        for n in 0..2 {
            let pixels=image::RgbaImage::from_pixel(32,32,image::Rgba([n*100,40,40,255]));
            let texture=ctx.load_texture(format!("compare_test_{n}"),ColorImage::from_rgba_unmultiplied([32,32],pixels.as_raw()),TextureOptions::LINEAR);
            app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels,texture,seed:0});
        }
        app.generation_workspace=true;
        app.generation_compare=true;
        app.generation_compare_index=1;
        app.generation_zoom=2.0;
        app.generation_pan=egui::vec2(17.0,-23.0);
        let frame=|app:&mut PixelForge,events| {
            let input=egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1400.0,900.0))),events,..Default::default()};
            let _=ctx.run(input,|ctx| {egui::CentralPanel::default().show(ctx,|ui|app.show_generation_workspace(ui));});
        };
        frame(&mut app,vec![]);
        let pointer=|pos,pressed| egui::Event::PointerButton {pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
        let start=egui::pos2(700.0,350.0);
        frame(&mut app,vec![egui::Event::PointerMoved(start),pointer(start,true)]);
        let end=egui::pos2(900.0,350.0);
        frame(&mut app,vec![egui::Event::PointerMoved(end)]);
        assert!(app.generation_compare_position>0.6,"Divider did not move: {}",app.generation_compare_position);
        frame(&mut app,vec![pointer(end,false)]);
        assert_eq!(app.generation_pan,egui::vec2(17.0,-23.0));
        for tab in 0..4 {
            app.generation_selected=tab%2;
            app.generation_tools_tab=tab==0;
            app.ai_tools_tab=tab==1;
            app.selection_tools_tab=tab==2;
            app.generation_compare=tab%2==0;
            frame(&mut app,vec![]);
            assert!((app.generation_zoom-2.0).abs()<0.001);
            assert_eq!(app.generation_pan,egui::vec2(17.0,-23.0));
            assert_eq!(app.workspace_editor,Some(tab%2));
        }
    }

    #[test]
    fn removing_arbitrary_version_remaps_working_comparison_and_parents() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        for n in 0..3 {
            let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([n,40,40,255]));
            let texture=ctx.load_texture(format!("remove_{n}"),ColorImage::from_rgba_unmultiplied([2,1],pixels.as_raw()),TextureOptions::LINEAR);
            let mut settings=settings_from_json(&serde_json::json!({}));
            settings.refine_parent=if n>0 {Some(n as usize-1)} else {None};
            app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings,pixels,texture,seed:n as u64});
        }
        app.generation_selected=2;app.generation_compare_index=1;app.generation_compare=true;
        app.bind_workspace_editor(&ctx);
        app.generation_zoom=2.0;app.generation_pan=egui::vec2(19.0,25.0);
        app.remove_workspace_version(0);app.bind_workspace_editor(&ctx);
        assert_eq!(app.generation_selected,1);
        assert_eq!(app.generation_compare_index,0);
        assert_eq!(app.generation_gallery[0].settings.refine_parent,None);
        assert_eq!(app.generation_gallery[1].settings.refine_parent,Some(0));
        assert_eq!(app.generation_zoom,2.0);
        assert_eq!(app.generation_pan,egui::vec2(19.0,25.0));
        app.remove_workspace_version(1);app.bind_workspace_editor(&ctx);
        assert_eq!(app.generation_selected,0);assert!(!app.generation_compare);
        app.remove_workspace_version(0);
        assert!(app.generation_gallery.is_empty());assert!(app.original_pixels.is_none());
        assert!(app.project_dirty);
    }

    #[test]
    fn thumbnail_caption_fits_and_negative_controls_are_visible() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        app.prompt_spelling.enabled=false;
        app.generation_auto_negative=true;
        app.generation_checkpoint="Fixture generation model".into();
        app.generation_prompt="Fixture initial prompt".into();
        let pixels=image::RgbaImage::from_pixel(32,32,image::Rgba([40,40,40,255]));
        let texture=ctx.load_texture("caption_fixture",ColorImage::from_rgba_unmultiplied([32,32],pixels.as_raw()),TextureOptions::LINEAR);
        app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels,texture,seed:0});
        let input=||egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.0,700.0))),..Default::default()};
        let output=ctx.run(input(),|ctx| {egui::CentralPanel::default().show(ctx,|ui|app.show_generation_workspace(ui));});
        let captions:Vec<_>=output.shapes.iter().filter_map(|clipped|match &clipped.shape {
            egui::Shape::Text(text) if text.galley.job.text=="Version 1"=>Some((text,clipped.clip_rect)),_=>None,
        }).collect();
        assert!(!captions.is_empty());
        for (text,clip) in captions {
            assert!(text.pos.y+text.galley.size().y<=clip.bottom()+1.0,"Thumbnail caption clipped");
        }
        let output=ctx.run(input(),|ctx| {egui::CentralPanel::default().show(ctx,|ui|app.show_conversation(ui));});
        let text:Vec<_>=output.shapes.iter().filter_map(|clipped|match &clipped.shape {
            egui::Shape::Text(text)=>Some((text.galley.job.text.as_str(),text.pos.y)),_=>None,
        }).collect();
        assert!(text.iter().any(|(s,_)|*s=="Use automatic negative prompts"));
        assert!(text.iter().any(|(s,_)|s.starts_with("Automatic (SDXL):")));
        let model_y=text.iter().find(|(s,_)|*s=="Fixture generation model").unwrap().1;
        let prompt_y=text.iter().find(|(s,_)|*s=="Fixture initial prompt").unwrap().1;
        assert!(model_y<prompt_y);
    }

    #[test]
    fn selected_area_overlay_is_drawn_outside_selection_tab_and_in_compare() {
        let ctx=egui::Context::default();let mut app=PixelForge::default();
        let pixels=image::RgbaImage::from_pixel(32,32,image::Rgba([40,40,40,255]));
        let texture=ctx.load_texture("overlay_fixture",ColorImage::from_rgba_unmultiplied([32,32],pixels.as_raw()),TextureOptions::LINEAR);
        app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({})),pixels,texture,seed:0});
        app.generation_workspace=true;app.bind_workspace_editor(&ctx);
        app.selection_mask=Some(masking::SelectionMask::from_pixels(32,32,vec![255;1024]).unwrap());
        app.refresh_selection_overlay(&ctx);
        let overlay=app.selection_texture.as_ref().unwrap().id();
        for tab in 0..4 {
            app.generation_tools_tab=tab==0;app.ai_tools_tab=tab==1;app.selection_tools_tab=tab==2;
            app.generation_compare=tab==3;
            let input=egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1400.0,900.0))),..Default::default()};
            let output=ctx.run(input,|ctx| {egui::CentralPanel::default().show(ctx,|ui|app.show_generation_workspace(ui));});
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Mesh(mesh) if mesh.texture_id==overlay)),"Selection missing from tab {tab}");
        }
    }

    #[test]
    fn generation_controls_fit_narrow_sidebar_with_expanded_settings() {
        let ctx=egui::Context::default();let mut app=PixelForge::default();
        app.prompt_spelling.enabled=false;
        app.edit_checkpoint=ai_generation::QWEN_EDIT.into();
        app.generation_checkpoint="CyberRealisticPony_V18.0_F16.safetensors".into();
        app.generation_custom_size=true;app.generation_unlock_dimensions=true;
        for width in [270.0,300.0,330.0] {
            let input=egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width,2400.0))),..Default::default()};
            let output=ctx.run(input,|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {
                    ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
                    ui.style_mut().animation_time=0.0;
                    for style in [egui::TextStyle::Body,egui::TextStyle::Button,egui::TextStyle::Small] {
                        ui.style_mut().text_styles.get_mut(&style).unwrap().size=16.0;
                    }
                    ui.spacing_mut().icon_width=22.0;
                    ui.spacing_mut().slider_width=width-100.0;
                    let right=ui.max_rect().right();
                    app.show_conversation(ui);
                    ui.indent("expanded_settings",|ui|app.show_generation_settings(ui));
                    assert!(ui.min_rect().right()<=right+1.0,"Sidebar width {width}: content right {} exceeds {right}",ui.min_rect().right());
                });
            });
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="CFG")),"Expanded settings were not rendered");
        }
    }

    #[test]
    fn studio_project_save_reopen_keeps_adjustments_masks_and_original_pixels() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([40,40,40,255]));
        let texture=ctx.load_texture("saved_test",ColorImage::from_rgba_unmultiplied([2,1],pixels.as_raw()),TextureOptions::LINEAR);
        app.generation_gallery.push(GeneratedItem {group:None, editor:studio::EditorState::default(),settings:settings_from_json(&serde_json::json!({"prompt":"Test"})),pixels,texture,seed:42});
        app.generation_workspace=true;app.bind_workspace_editor(&ctx);
        app.exposure=1.0;
        app.selection_mask=Some(masking::SelectionMask::from_pixels(2,1,vec![255,0]).unwrap());
        let path=std::env::temp_dir().join(format!("studio-workspace-roundtrip-{}.pxf",std::process::id()));
        app.qwen_steps=13;app.flux_steps=5;
        app.generation_project_path=Some(path.clone());
        app.project_dirty=true;
        // The exit dialog calls this entry point, with no legacy editor save path.
        app.save_project(false);
        assert!(!app.project_dirty);
        assert_eq!(app.generation_project_path.as_ref(),Some(&path));
        app.exposure=0.5;
        app.project_dirty=true;
        app.save_project(false);
        assert!(!app.project_dirty);
        assert!(app.generation_status.starts_with("Saved"),"{}",app.generation_status);
        let mut loaded=PixelForge::default();
        loaded.open_generation_project(&path,&ctx);loaded.ensure_workspace(&ctx);
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.qwen_steps,13);assert_eq!(loaded.flux_steps,5);
        assert_eq!(loaded.exposure,0.5);
        assert_eq!(loaded.selection_mask.as_ref().unwrap().pixels(),&[255,0]);
        assert_eq!(loaded.generation_gallery[0].pixels.get_pixel(0,0)[0],40);
        assert_eq!(loaded.generation_gallery[0].seed,42);
        assert_eq!(loaded.generation_gallery[0].settings.prompt,"Test");
    }
    #[test]
    fn explicit_blank_project_always_adds_a_tab_and_stays_empty() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        let initial=app.active_tab;
        app.create_blank_project_tab();
        assert_ne!(app.active_tab,initial);
        assert_eq!(app.inactive_tabs.len(),1);
        app.create_blank_project_tab();
        assert_eq!(app.inactive_tabs.len(),2);
        let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([40,40,40,255]));
        app.blank_project=false;
        app.texture=Some(ctx.load_texture("blank_tab_source",ColorImage::from_rgba_unmultiplied([2,1],pixels.as_raw()),TextureOptions::LINEAR));
        app.original_pixels=Some(pixels);
        app.generation_workspace=false;
        app.image_size=[2,1];
        app.ensure_workspace(&ctx);
        app.project_dirty=true;
        let populated=app.active_tab;
        app.create_blank_project_tab();
        let blank=app.active_tab;
        app.ensure_workspace(&ctx);
        assert!(app.blank_project);
        assert!(app.original_pixels.is_none());
        assert!(app.workspace_editor.is_none());
        assert!(!app.project_dirty);
        app.switch_project_tab(populated);
        app.ensure_workspace(&ctx);
        assert!(!app.blank_project);
        assert!(app.original_pixels.is_some());
        assert!(app.project_dirty);
        app.switch_project_tab(blank);
        app.ensure_workspace(&ctx);
        assert!(app.blank_project);
        assert!(app.original_pixels.is_none());
    }

    #[test]
    fn plain_image_enters_workspace_once_and_documents_keep_their_own_selection() {
        let ctx=egui::Context::default();
        let mut app=PixelForge::default();
        fn load(app:&mut PixelForge,ctx:&egui::Context,name:&str,value:u8) {
            let pixels=image::RgbaImage::from_pixel(2,1,image::Rgba([value,value,value,255]));
            app.texture=Some(ctx.load_texture(name,ColorImage::from_rgba_unmultiplied([2,1],pixels.as_raw()),TextureOptions::LINEAR));
            app.original_pixels=Some(pixels);app.preview_source=None;app.generation_workspace=false;
            app.image_size=[2,1];
        }
        load(&mut app,&ctx,"document1",40);app.exposure=1.0;
        app.ensure_workspace(&ctx);app.ensure_workspace(&ctx);
        assert_eq!(app.generation_gallery.len(),1);
        assert_eq!(app.exposure,1.0);
        let first=app.active_tab;
        app.create_project_tab();load(&mut app,&ctx,"document2",80);app.ensure_workspace(&ctx);
        assert_eq!(app.generation_gallery.len(),1);
        assert_eq!(app.workspace_editor,Some(0));
        assert_eq!(app.exposure, 0.0);
        app.switch_project_tab(first);app.ensure_workspace(&ctx);
        assert_eq!(app.workspace_editor,Some(0));
        assert_eq!(app.generation_selected,0);
        assert_eq!(app.exposure,1.0);
    }

    fn fixture_thumbnail(ctx: &egui::Context, seed: u8, parent: Option<usize>) -> GeneratedItem {
        let pixels = image::RgbaImage::from_pixel(2, 1, image::Rgba([seed, 0, 0, 255]));
        let texture = ctx.load_texture(
            format!("fixture_{seed}"),
            ColorImage::from_rgba_unmultiplied([2, 1], pixels.as_raw()),
            TextureOptions::LINEAR,
        );
        GeneratedItem {
            group: None,
            editor: studio::EditorState::default(),
            settings: settings_from_json(&serde_json::json!({
                "prompt": format!("Prompt {seed}"),
                "refine_parent": parent,
                "seed": seed,
            })),
            pixels,
            texture,
            seed: seed.into(),
        }
    }

    #[test]
    fn model_discovery_follows_the_app_while_project_model_choices_stay_local() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        let first = app.active_tab;
        app.generation_checkpoint = "Custom A.safetensors".into();
        let (tx, rx) = std::sync::mpsc::channel();
        app.generation_models_rx = Some(rx);
        app.create_blank_project_tab();
        let second = app.active_tab;
        assert!(app.generation_models_rx.is_some());

        tx.send(Ok(vec!["Custom A.safetensors".into(), "Custom B.safetensors".into()])).unwrap();
        app.poll_open_projects(&ctx);
        assert_eq!(app.generation_checkpoints.len(), 2);
        assert_eq!(app.generation_checkpoint, "Custom A.safetensors");
        app.generation_checkpoint = "Custom B.safetensors".into();

        app.switch_project_tab(first);
        assert_eq!(app.generation_checkpoints.len(), 2);
        assert_eq!(app.generation_checkpoint, "Custom A.safetensors");
        app.switch_project_tab(second);
        assert_eq!(app.generation_checkpoint, "Custom B.safetensors");
        app.create_blank_project_tab();
        assert_eq!(app.generation_checkpoints.len(), 2);
        assert_eq!(app.generation_checkpoint, "Custom A.safetensors");
    }

    #[test]
    fn async_generation_and_edit_stay_with_origin_across_switch_save_and_reopen() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        let origin = app.active_tab;
        let base = fixture_thumbnail(&ctx, 11, None);
        app.generation_batch_settings = Some(base.settings.clone());
        app.generation_gallery.push(base);
        app.generation_batch_settings
            .as_mut()
            .unwrap()
            .refine_parent = Some(0);
        app.generation_running = true;
        app.bind_workspace_editor(&ctx);
        app.exposure = 0.5;
        let (tx, rx) = std::sync::mpsc::channel();
        app.generation_rx = Some(rx);

        app.create_blank_project_tab();
        let other = app.active_tab;
        app.blank_project = false;
        app.generation_gallery
            .push(fixture_thumbnail(&ctx, 22, None));
        app.bind_workspace_editor(&ctx);
        app.exposure = 1.5;
        tx.send(Ok((fixture_thumbnail(&ctx, 33, None).pixels, 33)))
            .unwrap();
        app.poll_open_projects(&ctx);
        assert_eq!(app.active_tab, other);
        assert_eq!(app.generation_gallery.len(), 1);
        assert_eq!(app.generation_gallery[0].seed, 22);

        let root =
            std::env::temp_dir().join(format!("pixelforge-isolation-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let other_path = root.join("other.pxf");
        app.generation_project_path = Some(other_path.clone());
        app.save_generation_project(false);
        app.switch_project_tab(origin);
        assert_eq!(app.generation_selected, 1);
        assert_eq!(app.generation_gallery[0].editor.global.exposure, 0.5);
        assert_eq!(app.generation_gallery[1].settings.refine_parent, Some(0));
        assert_eq!(app.generation_gallery[1].settings.prompt, "Prompt 11");
        let origin_path = root.join("origin.pxf");
        app.generation_project_path = Some(origin_path.clone());
        app.save_generation_project(false);

        let mut reopened = PixelForge::default();
        reopened.open_generation_project(&origin_path, &ctx);
        let reopened_origin = reopened.active_tab;
        reopened.open_generation_project(&other_path, &ctx);
        assert_eq!(reopened.generation_gallery.len(), 1);
        assert_eq!(reopened.generation_gallery[0].editor.global.exposure, 1.5);
        reopened.switch_project_tab(reopened_origin);
        assert_eq!(reopened.generation_gallery.len(), 2);
        assert_eq!(reopened.generation_gallery[1].seed, 33);
        assert_eq!(reopened.generation_gallery[1].pixels.get_pixel(0, 0)[0], 33);
        assert_eq!(reopened.generation_gallery[0].editor.global.exposure, 0.5);
        std::fs::remove_file(origin_path).unwrap();
        std::fs::remove_file(other_path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn photo_tool_result_joins_only_the_project_that_started_it() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        let origin = app.active_tab;
        app.generation_gallery
            .push(fixture_thumbnail(&ctx, 12, None));
        app.bind_workspace_editor(&ctx);
        app.workspace_ai_parent = Some(0);
        app.ai_running = true;
        app.ai_strength = 1.0;
        let (tx, rx) = std::sync::mpsc::channel();
        app.ai_rx = Some(rx);
        app.create_blank_project_tab();

        let work_dir =
            std::env::temp_dir().join(format!("pixelforge-photo-owner-{}", std::process::id()));
        std::fs::create_dir_all(&work_dir).unwrap();
        let ai_path = work_dir.join("result.png");
        let pixels = fixture_thumbnail(&ctx, 91, None).pixels;
        pixels.save(&ai_path).unwrap();
        tx.send(Ok(ai_upscale::UpscaleResult {
            work_dir,
            ai_path: ai_path.clone(),
            baseline_path: ai_path,
            preview: pixels.clone(),
            baseline_preview: pixels,
            dimensions: [2, 1],
        }))
        .unwrap();
        app.poll_open_projects(&ctx);
        assert!(app.generation_gallery.is_empty());
        assert!(app.ai_variations.is_empty());
        app.switch_project_tab(origin);
        assert_eq!(app.generation_gallery.len(), 2);
        assert_eq!(app.generation_gallery[1].pixels.get_pixel(0, 0)[0], 91);
        assert_eq!(app.generation_gallery[1].settings.refine_parent, Some(0));
        assert_eq!(app.ai_variations.len(), 1);
    }

    #[test]
    fn thumbnail_wheel_scrolls_filmstrip_without_zooming_canvas() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        app.generation_gallery = (1..=10)
            .map(|seed| fixture_thumbnail(&ctx, seed, None))
            .collect();
        let mut scroll_id = None;
        let mut frame = |app: &mut PixelForge, events| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    scroll_id = Some(
                        ui.make_persistent_id(egui::Id::new(("studio_filmstrip", app.active_tab))),
                    );
                    app.show_generation_workspace(ui);
                });
            })
        };
        let output = frame(&mut app, vec![]);
        let caption = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Version 3" => Some(text.pos),
                _ => None,
            })
            .expect("visible thumbnail caption");
        let zoom = app.generation_zoom;
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(caption - egui::vec2(0.0, 40.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -100.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            egui::scroll_area::State::load(&ctx, scroll_id.unwrap())
                .unwrap()
                .offset
                .x
                > 0.0
        );
        assert_eq!(app.generation_zoom, zoom);
    }

    #[test]
    fn dragging_thumbnail_to_group_header_moves_it_into_group() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        app.thumbnail_groups.push(ThumbnailGroup {
            name: "Destinations".into(),
            collapsed: true,
        });
        app.generation_gallery = vec![fixture_thumbnail(&ctx, 1, None)];
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 640.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_generation_workspace(ui));
                },
            )
        };
        let output = frame(vec![]);
        let text_position = |label: &str| {
            output
                .shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text.contains(label) => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap()
        };
        let from = text_position("Version 1") - egui::vec2(0.0, 40.0);
        let to = text_position("Destinations");
        let click = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(from), click(from, true)]);
        frame(vec![egui::Event::PointerMoved(to)]);
        frame(vec![click(to, false)]);
        assert_eq!(app.generation_gallery[0].group, Some(0));
        assert!(app.project_dirty);
    }

    #[test]
    fn closing_origin_drops_its_worker_without_touching_the_next_project() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        let origin = app.active_tab;
        let (tx, rx) = std::sync::mpsc::channel();
        app.generation_rx = Some(rx);
        app.generation_running = true;
        let cancel = app.generation_cancel.clone();
        app.close_project_tab(origin);
        assert_ne!(app.active_tab, origin);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert!(
            tx.send(Ok((fixture_thumbnail(&ctx, 1, None).pixels, 1)))
                .is_err()
        );
        app.poll_open_projects(&ctx);
        assert!(app.generation_gallery.is_empty());
    }

    #[test]
    fn groups_reorder_copy_delete_and_roundtrip_without_changing_the_source() {
        let ctx = egui::Context::default();
        let mut app = PixelForge::default();
        let source = app.active_tab;
        app.thumbnail_groups = vec![
            ThumbnailGroup {
                name: "Drafts".into(),
                collapsed: false,
            },
            ThumbnailGroup {
                name: "Finals".into(),
                collapsed: true,
            },
        ];
        app.generation_gallery = vec![
            fixture_thumbnail(&ctx, 1, None),
            fixture_thumbnail(&ctx, 2, Some(0)),
            fixture_thumbnail(&ctx, 3, Some(1)),
        ];
        app.generation_gallery[1].group = Some(0);
        app.generation_gallery[2].group = Some(1);
        app.generation_selected = 2;
        app.generation_compare_index = 1;
        app.bind_workspace_editor(&ctx);
        app.move_thumbnail(ThumbnailDrop {
            index: 2,
            group: Some(0),
            before: 1,
        });
        let seeds = |app: &PixelForge| {
            app.generation_gallery
                .iter()
                .map(|item| item.seed)
                .collect::<Vec<_>>()
        };
        assert_eq!(seeds(&app), [1, 3, 2]);
        assert_eq!(app.generation_selected, 1);
        assert_eq!(app.workspace_editor, Some(1));
        assert_eq!(app.generation_compare_index, 2);
        assert_eq!(app.generation_gallery[1].settings.refine_parent, Some(2));

        let path =
            std::env::temp_dir().join(format!("pixelforge-groups-{}.pxf", std::process::id()));
        app.generation_project_path = Some(path.clone());
        app.save_generation_project(false);
        let mut loaded = PixelForge::default();
        loaded.open_generation_project(&path, &ctx);
        assert_eq!(seeds(&loaded), [1, 3, 2]);
        assert_eq!(loaded.thumbnail_groups, app.thumbnail_groups);
        assert_eq!(loaded.generation_gallery[1].group, Some(0));
        std::fs::remove_file(path).unwrap();

        let source_texture = app.generation_gallery[1].texture.id();
        app.open_thumbnails_as_project(&ctx, &[1, 2], Some(0));
        assert!(app.project_dirty && app.generation_project_path.is_none());
        assert_eq!(seeds(&app), [3, 2]);
        assert_eq!(app.thumbnail_groups[0].name, "Drafts");
        assert_ne!(app.generation_gallery[0].texture.id(), source_texture);
        assert_eq!(app.generation_gallery[0].settings.refine_parent, Some(1));
        assert_eq!(app.generation_gallery[1].settings.refine_parent, None);
        assert_eq!(app.generation_gallery[0].settings.prompt, "Prompt 3");
        app.delete_thumbnail_group(0, true);
        assert_eq!(seeds(&app), [3, 2]);
        assert!(
            app.generation_gallery
                .iter()
                .all(|item| item.group.is_none())
        );

        app.switch_project_tab(source);
        assert!(!app.project_dirty);
        assert_eq!(seeds(&app), [1, 3, 2]);
        app.open_thumbnails_as_project(&ctx, &[1, 2], None);
        assert!(app.thumbnail_groups.is_empty());
        assert_eq!(seeds(&app), [3, 2]);
        assert!(
            app.generation_gallery
                .iter()
                .all(|item| item.group.is_none())
        );
        app.switch_project_tab(source);
        app.delete_thumbnail_group(0, false);
        assert_eq!(seeds(&app), [1]);
        assert_eq!(app.thumbnail_groups[0].name, "Finals");
    }
}
