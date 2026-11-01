# PixelForge — AI Restoration and Generation Foundation

## Goal

Build **working, independently testable** AI sharpen, AI deblur, and image-generation pipelines without substituting traditional filters for neural inference.

## Current verified foundation

- Real-ESRGAN Vulkan AI upscaling (2x and 4x)
- Waifu2x Vulkan denoising (1x)
- Automatic discrete GPU preference, cancel, preview, strength blend, full-resolution save
- Selection masks and PXF project persistence

## Restoration operations

### AI Sharpen

- Purpose: recover perceptual edge/texture clarity on mildly soft photos.
- Requires a separately verified, licensed **neural restoration model** and inference backend.
- Strength 0–100% blends the neural result against the adjusted original.
- Keep input/output dimensions equal.
- Show Original / Sharpened comparison; retain full-resolution source and result.
- Avoid aggressive synthetic texture creation. Model choice and provenance should be visible.

### AI Deblur

- Purpose: recover mildly out-of-focus or motion-blurred photos.
- Prefer a model specifically trained for deblurring rather than treating sharpening as deblurring.
- Use the same cancellation, GPU selection, original-size comparison, and save pipeline.
- Warn users that severely missing details cannot be reliably reconstructed.
- Model-dependent padding, tiling, and VRAM safeguards are required.

### Model/backend acceptance checklist

1. Confirm exact upstream model, architecture, inference runtime, license, weights, and reproducible install procedure.
2. Confirm Windows execution on a Vulkan or compatible GPU path; provide CPU fallback only if viable.
3. Confirm model input/output format, color channels, image dimensions, and supported tile sizes.
4. Confirm real cancellation (terminate child process) and bounded diagnostic logging.
5. Test with user photographs; verify that outputs are neural inference and not conventional filters.
6. Ship or document redistribution of engine and model weights; do not embed unverified third-party binaries.

## Image generation foundation

Implement a **local inference provider** interface rather than hard-coding a single model:

- Text-to-image: prompt, negative prompt where supported, model, seed, dimensions, guidance, steps, and output preview.
- Image-to-image: input image, prompt, denoise/variation strength, seed.
- Inpainting: source image + existing PixelForge selection mask + prompt; preserve unmasked regions.
- Backend: start with a locally hosted diffusion server exposing a documented API, then optionally add direct native execution.
- Progress, cancellation, error diagnostics, VRAM handling, and output import/save.
- Save generation metadata (prompt, model identifier, seed, steps, guidance) with the project where supported.
- Explicit user control over downloaded models and their disk/VRAM requirements.

## Build order

1. Confirm compatible neural sharpen and deblur backends and weights.
2. Implement sharpen/deblur operations using a shared restoration runner and existing AI preview UI.
3. Validate on Windows with real photos.
4. Implement generation provider and text-to-image panel.
5. Add image-to-image and masked inpainting, then iterate on usability and output quality.

**Status:** Architecture and development branch established. Sharpen, deblur, and image generation are not yet implemented or validated.
