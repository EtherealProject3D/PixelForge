# Qwen Edit setup

Open **Generate → Model Library** in PixelForge. The library contains all curated
generation and editing models, with the same Download & Install action, progress,
cancel control, and installed status.

Choose the **models** folder belonging to the active ComfyUI installation if it
cannot be detected automatically. Select **Qwen-Image-Edit 2509 (FP8)** and click
**Download & Install**. It installs the editor, text encoder, and VAE (about 30 GB)
in their respective subfolders. Existing complete files are reused, including
shared encoder/VAE files. Incomplete or unexpectedly sized existing files are
reported for inspection rather than overwritten. File sizes are validated;
cryptographic hashes are not currently checked.

After installation, refresh installed models; restart ComfyUI if it has not yet
registered the new files. Click **Use for Edits** to choose Qwen under Changes
prompt. Generation model selection stays independent. ComfyUI must support the
Qwen nodes; update ComfyUI if the app reports missing nodes.

Required files:

- diffusion_models/qwen_image_edit_2509_fp8_e4m3fn.safetensors
- text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors
- vae/qwen_image_vae.safetensors

The app handles downloads in the background. Cancellation/failure removes the
active partial download; already completed bundle files are kept for retrying.
The optional scripts/Install-QwenEdit.ps1 remains available for command-line setup.

Apply Edit changes the whole image. Apply Edit to Selection retains only the
selected region of Qwen's result, with inward feathering at the boundary. Qwen
works on the intact image internally. Output returns to the source pixel size.
GPU memory use, speed, and quality depend on the hardware and chosen steps.

Model files and licenses: [Qwen editor](https://huggingface.co/Comfy-Org/Qwen-Image-Edit)
and [text encoder/VAE](https://huggingface.co/Comfy-Org/Qwen-Image).
