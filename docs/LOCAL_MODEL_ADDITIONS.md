# Local Qwen 2511 and FLUX Klein models

Both models are available in **Generate → Model Library → Download & Install**.
They use local ComfyUI inference and public model downloads; no API key or cloud
image-generation service is required. PixelForge does not download the bundles
until you choose the install action.

## Qwen-Image-Edit 2511 (FP8)

The library installs `qwen_image_edit_2511_fp8mixed.safetensors` and reuses the
existing Qwen text encoder and VAE when present. This needs about 20.5 GB of
additional disk space if Qwen 2509 is already installed. Otherwise the complete
bundle is about 30 GB. The older 2509 model remains available independently.

Click **Use for Edits** after installation. It selects the 2511 workflow, starts
with the recommended 40 steps, uses CFG 4 and sampling shift 3.1. The Changes
prompt's Qwen step slider remains editable. Selection edits use the same protected
pixel compositing and inward feathering as the existing editor.

## FLUX.2 Klein 4B (FP8)

The library installs:

- diffusion_models/flux-2-klein-4b-fp8.safetensors (about 4.1 GB)
- text_encoders/qwen_3_4b.safetensors (about 8.0 GB)
- vae/flux2-vae.safetensors (about 336 MB)

The complete bundle is about 12.5 GB. The card has both **Use for Generation** and
**Use for Edits**. It uses the distilled four-step workflow with CFG fixed at 1.
Negative prompts have no effect on this model, which is stated in the UI.
Generation steps and edit steps are independent; both are saved in projects.

Editing uses reference-latent conditioning, an approximately one-megapixel working
image, and restores the source dimensions afterwards. The implementation uses
FLUX.2-specific latents and scheduler, not an SDXL image-to-image graph.

ComfyUI may offload models between RAM and VRAM. Actual speed and memory use on
the 12 GB laptop GPU remain unverified. Update ComfyUI if the app reports missing
FLUX.2 nodes, encoder support, or model files. No fallback model is silently used.

Both new bundles support installed status, partial-file reuse, progress,
cancellation, and model-removal previews. Shared text encoders/VAEs are protected
by default when removing a bundle.

Sources:

- [Official Qwen workflow](https://github.com/Comfy-Org/workflow_templates/blob/main/templates/image_qwen_image_edit_2511.json)
- [Qwen ComfyUI files](https://huggingface.co/Comfy-Org/Qwen-Image-Edit/tree/main/split_files/diffusion_models)
- [Official FLUX Klein guide](https://docs.comfy.org/tutorials/flux/flux-2-klein)
- [Official FLUX FP8 model](https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8)
- [FLUX encoder and VAE](https://huggingface.co/Comfy-Org/Flux-2-Klein-4B)

Validation: local workflow graph/routing tests, setup-validation tests, bundle-file
sharing checks, project settings roundtrip, and Windows release build pass.
Model weights were intentionally not downloaded and live GPU inference was not run.
