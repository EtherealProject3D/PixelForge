# Dependencies and model storage

Open **Dependencies** on the top menu to see the setup inventory. Missing required
components highlight the menu in yellow with a count. On the first launch with
missing components the setup window opens automatically. Qwen is optional and
does not keep the required-dependency warning active by itself.

The inventory covers:

- ComfyUI generation backend, including existing ComfyUI Desktop installations.
- Waifu2x executable and the four noise-only model levels.
- Real-ESRGAN executable plus photo and illustration/anime weights.
- NAFNet deblur ONNX model.
- Python, OpenCV, NumPy, and ONNX Runtime for CPU deblurring (versions displayed).
- Vulkan GPU driver availability.
- An image-generation checkpoint and the optional Qwen editing bundle.

## Install, update, and roll back

**Install** or **Check / Install Update** prepares a new, separate installation.
Downloads and setup run in the background with progress, status, and cancellation.
The current installation remains selected unless setup and basic checks succeed.
Generation and AI processing cannot start during dependency maintenance.
Finish or cancel setup before closing the application.

New installations live under `%LOCALAPPDATA%\PixelForge\deps`, using short paths
for Windows systems without long-path support. Each dependency has version folders
and a small activation record. **Roll Back** switches to the previous saved version;
it can also switch back to the newer version. A first update can return to the
untouched original installation. Rollback is disabled until a previous installation
exists. Retained installations consume disk space; model weights are kept separately
from ComfyUI runtime snapshots.

ComfyUI updates install an official **portable** package, with its embedded Python
and GPU runtime. Choose the package matching your GPU before installing. This does
not update or modify ComfyUI Desktop. PixelForge stops only a backend it started;
an independently running backend must be stopped/restarted separately to switch
runtime versions. Portable backend model paths reuse Desktop's configured shared
model paths when present, and also support PixelForge's separate managed models folder.

The deblur setup installs an isolated embedded Python and binary packages. It does
not update system Python or its packages. NAFNet uses the supported 2025may model;
its update action repairs/installs that compatible model rather than selecting an
unverified architecture. The deblur runner is embedded in the app for first-time
installations that do not have the source tree beside the executable.

SHA256 is verified for Python, NAFNet, and release archives that publish a digest.
Other downloads use their official HTTPS release endpoints. GPU drivers remain
vendor-managed; the inventory links to setup guidance rather than replacing drivers.

## Remove unused models

Open **Generate → Model Library → Remove Model…**. The confirmation lists exact
files and the disk space that will be freed. **Remove These Files** permanently
deletes those files; Cancel keeps them. No images, projects, or history are deleted.

For Qwen, only the large editor file is selected by default. The checkbox to also
remove the shared text encoder and VAE is off, since other workflows may use them.
Partially installed bundles also have removal controls, allowing leftover shared
files to be removed deliberately. A model can be downloaded again later.

Removal is disabled during processing or downloads. PixelForge unloads a backend
it owns before removal. If another application holds a model file open, close that
application and retry. Only files inside the chosen model folder can be removed.

## Validation and remaining limits

- 90 regular automated tests pass, covering snapshot activation/rollback,
  incomplete installs, cancellation, model-file confinement, shared Qwen file
  protection, and existing image workflows.
- A separate opt-in integration test downloaded and staged official Waifu2x,
  Real-ESRGAN (including missing weights), NAFNet, and Python/package dependencies
  in disposable folders. A real 128×128 CPU deblur completed successfully.
- The Windows dependency inventory and model-removal preview were inspected.
  Existing Qwen files were left untouched; the 20.43 GB editor-only preview was cancelled.
- The ComfyUI portable download/extraction/activation path is implemented, but a
  complete portable installation and GPU generation run remain unverified.
- Rollback restores managed dependency folders and activation, not arbitrary
  external changes made by other software or GPU driver updates.

Sources: [ComfyUI portable](https://docs.comfy.org/installation/comfyui_portable_windows),
[Waifu2x](https://github.com/nihui/waifu2x-ncnn-vulkan/releases),
[Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/releases),
[full Real-ESRGAN weights](https://github.com/xinntao/Real-ESRGAN/releases/tag/v0.2.5.0),
[NAFNet](https://huggingface.co/opencv/deblurring_nafnet),
[Python](https://www.python.org/downloads/release/python-31210/),
and [7-Zip extractor](https://www.7-zip.org/download.html).
