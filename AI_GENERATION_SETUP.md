# Stage 3.5 — Local GPU Image Generation (first integration)

PixelForge now has an **Image Generation** section at the top of the AI Tools sidebar.
It communicates only with ComfyUI at `http://127.0.0.1:8188` and does not use a hosted image-generation API.

## Setup (Windows / NVIDIA)

1. Install ComfyUI locally from its official project: https://github.com/comfyanonymous/ComfyUI
2. Install a compatible local **Stable Diffusion 1.5 or SDXL checkpoint** into ComfyUI's `models/checkpoints` folder. The first integration uses the standard CheckpointLoaderSimple / CLIPTextEncode / KSampler / VAEDecode nodes. FLUX and other architectures requiring custom workflows are **not yet supported**.
3. Launch ComfyUI with NVIDIA GPU support and bind it to localhost, port 8188. Confirm `http://127.0.0.1:8188` opens on the same PC.
4. In PixelForge's AI Tools, click **Connect / Refresh Models**, choose a checkpoint, enter prompts, set dimensions (multiples of 64), and click **Generate**.
5. The generated image appears in the sidebar. Click **Open Generated Image in Editor** to edit it with PixelForge. Save/close any unsaved project first.

ComfyUI also saves its output in its configured output directory. PixelForge writes a temporary PNG when you open the generated image in the editor; use PixelForge's Save/Export workflow to keep a permanent edited copy.

## Current limits

- This is the first end-to-end implementation, **not yet compiled or tested on the user's PC**.
- Standard SD 1.5/SDXL workflows only. Model names are read from local ComfyUI.
- One image per generation, Euler sampler, normal scheduler.
- Generate and checkpoint discovery use the localhost HTTP API. Image data stays local.
- Cancel removes a *queued* prompt; a prompt already executing may finish in ComfyUI. PixelForge stops waiting for it.
- Generation can take several minutes; the UI polls for completion without blocking.
- Width and height must be multiples of 64 between 256 and 2048.
- The existing AI Result Strength slider applies to enhancement/restoration results, **not** text-to-image generation.
- Future milestones: progress reporting, richer samplers, generation history, image-to-image and mask-based inpainting.

## Build

```powershell
cd C:\PixelForge
git fetch origin
git switch dev/stage-3-5-local-generation
git pull --ff-only
cargo test
cargo run --release
```
