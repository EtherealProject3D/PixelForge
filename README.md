# PixelForge

### Create. Restore. Refine. Locally.

[![Developed with ChatGPT](https://img.shields.io/badge/Developed%20with-ChatGPT-10a37f?logo=openai&logoColor=white)](CONTRIBUTORS.md)

PixelForge is a Windows desktop image studio that brings local AI generation, conversational editing, photo restoration, and traditional adjustments into one workspace. Generate an image or import a photograph, make changes, and compare every result without switching between separate viewers.

**Version 0.6.7 · Windows x64 · Rust + egui · Local ComfyUI**

[Download v0.6.7](https://github.com/EtherealProject3D/PixelForge/releases/tag/v0.6.7) · [Report an issue](https://github.com/EtherealProject3D/PixelForge/issues) · [Restoration guide](docs/PHOTO_RESTORATION.md)

## One workspace, from first image to final export

**Edit**, **AI Tools**, **Generate**, and **Selection** share the same canvas and bottom version history. Switching tools keeps the working image, zoom, pan, and selection in place.

- **Project tabs:** keep several projects open with separate images, version histories, and settings. Background jobs return results to the project that started them.
- **Thumbnail groups:** Click a group header to select where new images and edits go. Press Escape to clear the group selection. Click its square to collapse or expand it, and right-click empty filmstrip space to create a blank group. Ctrl-click to select several versions, then right-click and choose **Add to New Group…**. Drag versions within or between groups, and drag group headers to reorder groups. Right-click to rename or remove a version. Version names stay with their images when you reorder them.
- **New versions:** edits join the selected group, falling back to the source image's group when no group is selected. Imports and new generations join the selected group or Ungrouped. Right-click a thumbnail or group to open it in an independent, unsaved project. When opening a group, choose whether to keep the group or open its images as individual thumbnails.
- **Sliding comparison:** keep a reference on the left and a working version on the right. New results update the right side while your reference stays selected.
- **Image viewing:** Use Left / Right or hover near the viewer edges to browse the current group. Hover at the bottom right for the full-screen button, or press Alt+Enter. In full screen, scroll to zoom, drag to pan, double-click to fit, and press Esc to return. Click the divider handles to hide or restore the sidebar and thumbnails.
- **Canvas navigation:** scroll over the canvas to zoom, or over the filmstrip to browse thumbnails. Pan and double-click to fit. Rotate a version in place by 90° to use that orientation in subsequent edits.
- **Project saving:** save images, versions, names, settings, adjustments, and selections in a `.pxf` project. Ctrl+S saves the active project. Open Project accepts multiple project files, each in its own tab.
- **Image export:** save a single image or use **Export → Batch Export** to save selected or all thumbnails as PNG, JPEG, or WebP.

## Local generation and conversational editing

Separate **Initial prompt**, **Negative prompt**, and **Changes prompt** areas make creation and refinement easy to follow.

- Independent generation and edit model selection.
- SDXL generation, image-to-image refinement, and masked inpainting that preserves pixels outside the mask.
- FLUX.2 Klein generation and instruction-based editing.
- Qwen-Image-Edit 2509 and 2511 instruction-based editing.
- Whole-image edits or **Apply Edit to Selection**, with feathered blending at the boundary.
- Styles, aspect ratios, custom resolution, batches, seeds, sampling steps, and model-specific defaults.
- Automatic SDXL negative prompts with editable additional exclusions.
- English spell checking with red underlines and right-click suggestions. Prompt editors keep highlighted text selected when you right-click to Cut or Copy.
- Use local AI Assist in **Prompt Guide** to revise a draft. Choose **Transfer to Initial Prompt** to use a suggestion and keep your scratchpad for further drafts. The Scratchpad opens by default. Saved user tags have a ◆ mark and can be dragged between categories.
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

Open **Setup → Model Library** to see installed models, manage downloads, and preview files before removal. Choose **Continue Download** when a supported download has saved progress. PixelForge suggests a model folder on first setup and remembers any custom folder you choose.

| Model | Supported workflows |
| --- | --- |
| SDXL Base 1.0 | Generation, refinement, inpainting |
| RealVisXL V5 | Generation, refinement, inpainting |
| Juggernaut XL v9 | Generation, refinement, inpainting |
| CyberRealisticPony v18 | Generation, refinement, inpainting |
| Pony Diffusion V6 XL | Generation |
| FLUX.1 Dev (FP8) | Generation with optional local LoRA support |
| CyberRealistic XL v10 FP16 | Generation, refinement, inpainting |
| CyberRealistic Z-Image Turbo v4 | Generation; 9 steps, CFG 1 |
| FLUX.2 Klein 4B FP8 | Generation, instruction editing, AI Restore |
| Qwen-Image-Edit 2509 FP8 | Instruction editing, AI Restore |
| Qwen-Image-Edit 2511 FP8 | Instruction editing, AI Restore |

Downloads are substantial: SDXL checkpoints are typically about 7 GB each, FLUX Klein's bundle about 12.5 GB, and a Qwen bundle about 30 GB. Qwen variants share a text encoder and VAE. Shared components are retained by default during removal.

Models and third-party engines are downloaded separately and remain subject to their publishers' licenses and terms. No cloud generation service or API key is required by the bundled workflows.

## What's new in v0.6.7

- Browse the current image's group with Left / Right or arrows that appear near the viewer edges.
- View an image full screen with Alt+Enter, zoom with the mouse wheel, drag to pan, and double-click to fit. Alt+Enter or Esc returns to the editor.
- Click the divider handles to hide or restore the sidebar and thumbnail strip. Group headers select the destination for new images and edits. Escape clears that selection.
- Open several projects at once, save the active project with Ctrl+S, and remove its selected image version with Delete.
- Start Ollama and ComfyUI in the background when PixelForge opens, and select models discovered on the connected backend.
- Use improved Danbooru tag wrapping and drag saved user tags between categories. Prompt Guide opens with the Scratchpad visible and restores its size and position.

See the [v0.6.7 release notes](docs/RELEASE_NOTES_v0.6.7.md) for details.

## Getting started on Windows

1. Choose a download from [Releases](https://github.com/EtherealProject3D/PixelForge/releases/tag/v0.6.7): **PixelForge-v0.6.7-windows-x64-setup.exe** for installation, or **PixelForge-v0.6.7-windows-x64.zip** for portable use.
2. The installer defaults to **Program Files\Pixel Forge**, adds a Start menu entry, and offers an optional desktop shortcut. Alternatively, extract the portable ZIP into a writable folder and open **PixelForge.exe**. Neither download requires Rust.
3. Open **Setup → Dependencies** to check and install the components needed by your tools. A count beside **Setup** shows missing required components.
4. For generation and editing, open **Setup → Setup & Diagnostics** to connect ComfyUI, choose Auto, GPU, or CPU, and run diagnostics. You can use a compatible ComfyUI Desktop installation or PixelForge's managed portable backend.
5. In **Setup → Model Library**, use the recommended folder or choose a custom one, install a model, and refresh installed models.
6. Generate or import an image. Use **AI Tools** for restoration, **Changes prompt** for edits, and **Compare** to evaluate results.
7. Save a `.pxf` project to retain history, or export a finished image.

### Requirements

- Windows x64; other platforms have not been validated for this release.
- Current GPU drivers. Waifu2x and Real-ESRGAN use Vulkan; ComfyUI hardware support depends on its installed backend configuration.
- Sufficient RAM, VRAM, and disk space for the selected model and image size. Large edit models can be slow or exceed memory. User testing includes an NVIDIA RTX 5070 Ti Laptop GPU; other configurations vary.
- Internet access for engine/model downloads. Inference runs locally after setup. ComfyUI defaults to `127.0.0.1:8188`.

### Dependency management

The **Dependencies** window reports status and offers supported install, update, and rollback actions for managed components. It includes ComfyUI, Ollama, Waifu2x, Real-ESRGAN, NAFNet, and Python. Managed updates use separate versions and activate after validation. GPU drivers remain vendor-managed; existing external installations are not replaced.

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
git switch --detach v0.6.7
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

Read [What's new in v0.6.7](docs/RELEASE_NOTES_v0.6.7.md) for the release overview. Windows automated tests cover processing helpers, AI workflow construction, version/selection state, comparison interactions, and project round trips. GPU speed and restoration quality vary by model and photo.

When [reporting an issue](https://github.com/EtherealProject3D/PixelForge/issues), include the version, GPU, model, operation, image dimensions, and exact error. Only attach photos you are comfortable sharing publicly.
