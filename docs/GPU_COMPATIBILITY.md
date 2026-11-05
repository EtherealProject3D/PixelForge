# GPU compatibility and safe backend selection (v0.6.3 development)

ComfyUI publishes separate modern NVIDIA and legacy CUDA 12.6 packages:
https://docs.comfy.org/installation/comfyui_portable_windows
PyTorch removed Pascal support from CUDA 12.8 binaries beginning in 2.8:
https://dev-discuss.pytorch.org/t/cuda-toolkit-version-and-architecture-support-update-maxwell-and-pascal-architecture-support-removed-in-cuda-12-8-and-12-9-builds/3128

Dependencies > GPU compatibility and backend selection provides Auto (the existing
managed installation or Desktop), Modern, and Legacy. Explicit modes use a folder
you register, never silently fall back. Extract an official compatible portable
package into a separate folder; do not run package upgrades inside the stable
environment. Selection is stored per profile, independent of shared activation.
No package changes occur when choosing a folder or running diagnostics.
The existing Install / Update and Roll Back controls still explicitly modify shared
managed dependencies; do not use them simply to test the new display controls.

A backend already running on port 8188 is reused. PixelForge never kills an external
server. Stop that server yourself before testing a different selection. The Stop
PixelForge-owned backend button only stops a process created by this app.
New owned NVIDIA launches probe the selected Python's PyTorch/CUDA, device capability,
and an actual small matrix operation. A mismatch stops before launching ComfyUI.
Legacy launches use FP32 and PyTorch attention to avoid optional modern precision /
attention kernels. This uses more memory and is slower. FP8 models and models requiring
newer operations may still be incompatible. Use conservative resolutions and a small
checkpoint for the eventual GTX 1080 trial.

GTX 1080 generation is UNVERIFIED until tested on physical hardware. Mock tests cover
Pascal / Blackwell selection, unsupported kernels, and missing explicit folders;
passing a CUDA probe does not prove a model can generate successfully.

Dependencies runs a background read-only diagnostic snapshot and offers refresh,
copy and save. It includes GPU / total and free VRAM / driver / capability,
the selected Python runtime and its kernel check, and any currently serving backend.
Those environments may differ. nvidia-smi's CUDA header is a driver capability limit,
not the installed Python CUDA version. Missing tools are reported as unavailable,
not as evidence that no GPU exists. Diagnostic subprocesses have timeouts.
Startup stdout/stderr is saved to backend.log in the existing profile folder.
Generation errors retain technical details and add specific recovery steps for
memory, architecture, driver and precision failures. No security settings are changed.
