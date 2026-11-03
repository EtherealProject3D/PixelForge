# PixelForge v0.6.0

Local creation, conversational editing, and photo restoration in one consistent Windows image workspace.

## Highlights

- Shared canvas and version thumbnails across Edit, AI Tools, Generate, and Selection.
- FLUX.2 Klein 4B FP8 generation/editing, Qwen-Image-Edit 2509/2511 FP8 editing, and SDXL generation/refinement/inpainting.
- Dedicated AI Restore with independent model, instructions, and sampling steps.
- Photo cleanup presets, Waifu2x denoise, NAFNet deblur, Real-ESRGAN sharpening and upscaling.
- Persistent left comparison reference and latest/selected result on the right, with a draggable divider.
- In-place rotation, inherited orientation, and double-click to fit across tabs.
- Thumbnail menus for naming and removing versions; names retained in projects and comparison menus.
- Separate initial, negative, and changes prompts with spell checking.
- Unified model downloads/removal and dependency installation, updates, and managed rollback.
- Refined dark theme, fixed-width sidebar, permanent scrollbar lane, and consistent save/exit behavior.

## Download and setup

Extract the Windows x64 ZIP and run **PixelForge.exe**. Use **Dependencies** and **Model Library** to install components. Model weights and external engines are not included. See the [README](../README.md) for setup and storage requirements.

## Validation and limits

Windows release builds and automated tests cover workflow construction, processing helpers, version/selection state, comparison, and project saving/reopening. User testing includes local FLUX editing and photo restoration. Full GPU inference and first-install behavior across all hardware/model combinations have not been certified. The opt-in dependency download/inference integration test is separate from the normal suite.

- Generative restoration may invent details; compare faces and important features with the original.
- Loading/inference depends on VRAM, RAM, image size, and backend configuration.
- NAFNet uses a separate Python runtime and may be slow on large photos.
- FLUX ignores negative prompts. Qwen/FLUX selection edits composite an internally edited image; SDXL uses inpainting.
- Managed rollback switches supported components between retained versions; GPU drivers and external installations remain independently managed.
- Save `.pxf` projects to retain history. Flat image exports do not contain the whole project.
