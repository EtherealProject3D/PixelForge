# Qwen-Image-Edit 2509 setup

PixelForge now has a dedicated local Qwen instruction-edit workflow, selectable
under Changes prompt. SDXL generation and SDXL whole-image/masked refinement remain
available. This Qwen integration uses native ComfyUI nodes, 20 Euler/simple steps,
CFG 4, shift 3, CFG normalization and the image-conditioned Qwen Edit Plus encoder.
It does not require the optional Lightning LoRA.

1. Update ComfyUI to a version supporting TextEncodeQwenImageEditPlus and Qwen Edit
   2509. Keep the backend on 127.0.0.1:8188.
2. Locate the models folder actually used by your active ComfyUI instance.
3. Install the three official model files (about 28 GB total; allow extra room):

```powershell
.\scripts\Install-QwenEdit.ps1 -ModelsDirectory 'C:\your\ComfyUI\models'
```

Replace the example path with your actual folder. Downloads resume using .part
files and existing completed files are not overwritten. The script checks minimum
file sizes, not cryptographic hashes. Run with -WhatIf to preview destinations.

Files and destinations:

- models/diffusion_models/qwen_image_edit_2509_fp8_e4m3fn.safetensors
- models/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors
- models/vae/qwen_image_vae.safetensors

Restart ComfyUI after installation. Create or import an image in PixelForge, select
Qwen-Image-Edit 2509 (FP8), enter a change instruction and Apply Edit. It sends the
selected image and that instruction, without concatenating previous SDXL prompts.
Qwen currently edits the whole image; cancel any painted area before using it.
Missing nodes/files produce actionable errors before the image upload or job
submission, with no silent SDXL fallback. Model filenames must match the paths above
at the root of each model category (not a nested subfolder).

The pipeline scales the image to the Qwen working size internally, then resamples the result back to the exact source dimensions with Lanczos filtering. This preserves pixel dimensions; it does not add native model detail. Metadata records the final dimensions and model/sampling settings. Only the bottom status panel shows generation progress; the installation-help link is tucked inside Generation settings. Generation model selection is
independent. Large FP8 models can still exceed laptop memory; GPU performance,
offloading and output quality must be tested on your hardware. Model weights are
not bundled, automatically downloaded at application startup, or installed by the
Rust build. A 15-minute per-job timeout remains in place.

References:
- https://comfyanonymous.github.io/ComfyUI_examples/qwen_image/
- https://github.com/Comfy-Org/workflow_templates/blob/main/templates/image_qwen_image_edit_2509.json
- https://huggingface.co/Comfy-Org/Qwen-Image-Edit_ComfyUI

## Ensure you are running the updated application

The older title PixelForge v0.5.0-dev and the Refine Selected Image button identify
the old executable. The new title is PixelForge v0.5.1-stage1.

Close old copies. Inside the intended repository:

```powershell
git fetch origin
git switch feature/stage-1-conversational-editing
git pull --ff-only
.\scripts\Run-PixelForge.ps1
```

This launcher builds and starts the executable from its own checkout, instead of a
pinned shortcut or copied executable elsewhere. Both Initial prompt and Changes
prompt should appear in the right sidebar, and thumbnails below the canvas.
