# PixelForge

### Create. Restore. Refine. Locally.

[![Developed with ChatGPT](https://img.shields.io/badge/Developed%20with-ChatGPT-10a37f?logo=openai&logoColor=white)](CONTRIBUTORS.md)

PixelForge is a Windows desktop image studio that brings local AI generation, conversational editing, photo restoration, and traditional adjustments into one workspace. Generate an image or import a photograph, make changes, and compare every result without switching between separate viewers.

**Version 0.6.3 · Windows x64 · Rust + egui · Local ComfyUI**

[Download v0.6.3](https://github.com/EtherealProject3D/PixelForge/releases/tag/v0.6.3) · [Report an issue](https://github.com/EtherealProject3D/PixelForge/issues) · [Restoration guide](docs/PHOTO_RESTORATION.md)

## One workspace, from first image to final export

**Edit**, **AI Tools**, **Generate**, and **Selection** share the same canvas and bottom version history. Switching tools keeps the working image, zoom, pan, and selection in place.

- **Version history:** generated images, AI edits, and cleanup results appear as thumbnails. Right-click to rename or remove a version; save manual adjustments as a new version when needed.
- **Sliding comparison:** keep a reference on the left and a working version on the right. New results update the right side while your reference stays selected.
- **Canvas navigation:** scroll to zoom, pan, and double-click to fit. Rotate a version in place by 90°; subsequent edits use its orientation.
- **Project saving:** save images, versions, names, settings, adjustments, and selections in a `.pxf` project. Save & Exit updates the existing file.
- **Image export:** export PNG, JPEG, or WebP independently of the project.

## Local generation and conversational editing

Separate **Initial prompt**, **Negative prompt**, and **Changes prompt** areas make creation and refinement easy to follow.

- Independent generation and edit model selection.
- SDXL generation, image-to-image refinement, and masked inpainting.
- FLUX.2 Klein generation and instruction-based editing.
- Qwen-Image-Edit 2509 and 2511 instruction-based editing.
- Whole-image edits or **Apply Edit to Selection**, with feathered blending at the boundary.
- Styles, aspect ratios, custom resolution, batches, seeds, sampling steps, and model-specific defaults.
- Automatic SDXL negative prompts with editable additional exclusions.
- English spell checking with red underlines and right-click suggestions.
- Bottom generation progress and cancellation, plus image settings and prompt details.

FLUX Klein uses CFG 1 and ignores negative prompts. SDXL selection edits use inpainting; Qwen and FLUX edit internally and composite the selected area back into the original. Edits return to the source pixel dimensions.

## Photo restoration and AI tools

| Tool | Purpose |
| --- | --- |
| AI Restore | Restore grainy or damaged photos with editable instructions, FLUX or Qwen, and independent sampling steps. |
| Denoise Photo | Reduce noise with Waifu2x at the original size. |
| Reduce Blur | Improve photographic blur with NAFNet. |
| Sharpen Details | Use Real-ESRGAN luminance detail while retaining original colors. |
| Upscale 2× / 4× | Enlarge photos or illustrations with Real-ESRGAN. |

Gentle, Grainy Photo, and Soft Photo presets provide cleanup starting points. Set the next cleanup's strength independently of the previous result's preview blend. Successful operations create versions automatically, preserving earlier results for comparison.

Generative restoration reconstructs plausible detail. Check faces, text, and important features against the original; a cleaner result does not prove that missing information was recovered accurately.

## Adjustments and selections

Adjust brightness, contrast, exposure, saturation, temperature, tint, highlights, shadows, whites, blacks, vibrance, and gamma. Work globally or within a selection, inspect the histogram and image analysis, or start with Auto Enhance.

Selection painting, undo/redo, and named saved selections are integrated into the shared workspace. Active selections remain visible across tabs. Rotation also rotates active and saved selections.

## Models in one library

The in-app **Model Library** provides installation status, downloads, progress, cancellation, and file-removal previews.

| Model | Supported workflows |
| --- | --- |
| SDXL Base 1.0 | Generation, refinement, inpainting |
| RealVisXL V5 | Generation, refinement, inpainting |
| Juggernaut XL v9 | Generation, refinement, inpainting |
| CyberRealisticPony v18 | Generation, refinement, inpainting |
| CyberRealistic XL v10 FP16 (v0.6.3 development) | Generation, refinement, inpainting |
| CyberRealistic Z-Image Turbo v4 (v0.6.3 development) | Generation; 9 steps, CFG 1 |
| FLUX.2 Klein 4B FP8 | Generation, instruction editing, AI Restore |
| Qwen-Image-Edit 2509 FP8 | Instruction editing, AI Restore |
| Qwen-Image-Edit 2511 FP8 | Instruction editing, AI Restore |

Downloads are substantial: SDXL checkpoints are typically about 7 GB each, FLUX Klein's bundle about 12.5 GB, and a Qwen bundle about 30 GB. Qwen variants share a text encoder and VAE. Shared components are retained by default during removal.

Models and third-party engines are downloaded separately and remain subject to their publishers' licenses and terms. No cloud generation service or API key is required by the bundled workflows.

## Getting started on Windows

1. Choose a download from [Releases](https://github.com/EtherealProject3D/PixelForge/releases/tag/v0.6.2): **PixelForge-v0.6.2-windows-x64-setup.exe** for installation, or **PixelForge-v0.6.2-windows-x64.zip** for portable use.
2. The installer defaults to **Program Files\Pixel Forge**, adds a Start menu entry, and offers an optional desktop shortcut. Alternatively, extract the portable ZIP into a writable folder and open **PixelForge.exe**. Neither download requires Rust.
3. Open **Dependencies** to check and install the components needed by your tools. Missing required components highlight that menu item.
4. For generation/editing, connect a compatible local ComfyUI Desktop installation or install PixelForge's managed portable backend.
5. In **Generate → Model Library**, choose the models folder if prompted, install a model, and refresh installed models.
6. Generate or import an image. Use **AI Tools** for restoration, **Changes prompt** for edits, and **Compare** to evaluate results.
7. Save a `.pxf` project to retain history, or export a finished image.

### Requirements

- Windows x64; other platforms have not been validated for this release.
- Current GPU drivers. Waifu2x and Real-ESRGAN use Vulkan; ComfyUI hardware support depends on its installed backend configuration.
- Sufficient RAM, VRAM, and disk space for the selected model and image size. Large edit models can be slow or exceed memory. User testing includes an NVIDIA RTX 5070 Ti Laptop GPU; other configurations vary.
- Internet access for engine/model downloads. Inference runs locally after setup. ComfyUI defaults to `127.0.0.1:8188`.

### Dependency management

The **Dependencies** window reports status and offers supported install, update, and rollback actions for managed ComfyUI, Waifu2x, Real-ESRGAN, the NAFNet model, and its Python runtime. Managed updates use separate versions and activate after validation. GPU drivers remain vendor-managed; existing external installations are not replaced.

### App updates (v0.6.1 onward)

Open **Updates → Check for Updates** to see newer published stable versions and their release notes. Download the verified package, save all open projects, finish running operations, then choose **Install & Restart**. Installed copies open the setup wizard and request Windows administrator permission; portable copies update in place in a writable folder. Both retain a previous executable backup. Users on v0.6.0 need to download a current release manually first. Source checkouts should continue using Git pull and rebuild; the development launcher rebuilds the checked-out code.

Installed copies can be removed using Windows **Installed apps**. Uninstall removes the application and its shortcuts and offers to remove PixelForge-managed downloaded models and dependencies to free disk space. Keeping downloads is the default. Projects and settings are retained.

See [Windows installation options](docs/WINDOWS_INSTALLATION.md) for installer, portable, update, and installer-build details.

See [Dependency setup and rollback](docs/DEPENDENCIES.md), [Photo restoration](docs/PHOTO_RESTORATION.md), and [Local model details](docs/LOCAL_MODEL_ADDITIONS.md).

## Build from source

Install Git, Rust stable, and Visual Studio C++ build tools with the Windows SDK. In PowerShell:

```powershell
git clone https://github.com/EtherealProject3D/PixelForge.git
Set-Location PixelForge
git switch --detach v0.6.2
cargo test --locked
cargo run --locked --release
```

To update a checkout tracking the published source:

```powershell
git switch main
git pull --ff-only
cargo build --locked --release
.\target\release\pixelforge.exe
```

Close PixelForge before rebuilding its executable. Source checkouts include `scripts/Launch-PixelForge.vbs`, a quiet launcher that builds the latest local checkout before opening the app. It does not pull from GitHub automatically. Diagnostics are saved under `target/launcher*.log`.

## Release notes and support

Read [What's new in v0.6.2](docs/RELEASE_NOTES_v0.6.2.md) for the release overview and validation limits. Windows automated tests cover processing helpers, AI workflow construction, version/selection state, comparison interactions, and project round trips. GPU speed and restoration quality vary by model and photo.

When [reporting an issue](https://github.com/EtherealProject3D/PixelForge/issues), include the version, GPU, model, operation, image dimensions, and exact error. Only attach photos you are comfortable sharing publicly.
