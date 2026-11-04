# PixelForge v0.6.3 — development, not approved for final release

- Settings > Display: native fullscreen, F11 toggle, Esc exit, persistent mode and
  normal window placement. Uses the current monitor rather than a fixed resolution.
- Dependencies: background GPU / VRAM / driver / compute capability and selected
  Python CUDA / PyTorch diagnostics; inspect the running server separately, copy or
  save the report, and read startup output in the profile's backend.log.
- CUDA failures include memory, architecture, driver and precision recovery advice
  while preserving technical details.
- Per-profile Auto / Modern / Legacy backend selection reuses existing separate
  folders without overwriting Python packages or shared activation records. Existing
  external servers remain under the user's control. New owned NVIDIA launches check
  a real small CUDA kernel before starting ComfyUI. Legacy uses conservative FP32 /
  PyTorch attention; GTX 1080 generation is unverified pending hardware testing.
- Generation settings suggest a model- and VRAM-aware resolution with one-click
  apply and unrestricted custom override. Advice is heuristic, not a memory guarantee.
- Local and CI packaging produce SHA-256 and signature inventory reports. CI prepares
  draft releases. Code signing is evaluated but not configured; warnings are not
  claimed resolved. See RELEASE_SECURITY.md.

Preserves stable v0.6.2 and existing development settings, model and ComfyUI folders.
The five pre-existing post-v0.6.2 model-library development commits were retained.

## Laptop acceptance test

1. Close any old PixelForge Dev window. Open the existing PixelForge Dev shortcut
   (or scripts/Run-PixelForge.ps1); confirm the title says PixelForge Dev v0.6.3.
2. Move and resize the window, select Settings > Display > Fullscreen. Confirm no
   title bar / taskbar, and full monitor coverage. F11 returns to the same placement.
3. Enter fullscreen again and use Esc. Repeat from a maximized window. Close while
   fullscreen, reopen, then Esc. Check that the earlier normal placement is restored.
4. At Windows 150% scaling and on a 32:9 monitor, repeat; change UI scale while in
   fullscreen and check the restored physical window size. Test each connected monitor.
5. Open Dependencies > GPU compatibility and backend selection. Keep Auto for your
   working RTX setup. Run diagnostics and confirm the serving GPU, memory and CUDA /
   PyTorch details. Copy the report if a failure occurs. Do not install dependencies
   merely to test display settings.
6. Generate one image with an already working checkpoint. In expanded Generation
   settings inspect the resolution suggestion, apply it, then choose custom dimensions
   to confirm override. Compare a small model and a large bundle if already installed.
7. Check that normal installed PixelForge v0.6.2 still opens from its normal shortcut.

Do not test Legacy on the RTX laptop by replacing its working environment. GTX 1080
hardware generation, actual ultrawide/scaling behavior and fresh-download Windows
trust prompts remain manual acceptance checks. No final v0.6.3 publication is approved.

## Validation completed on 2026-11-04

- 115 Rust tests passed, zero failures; one existing opt-in real dependency download
  test ignored. Tests ran serially with access to local mock servers and normal temp
  files; the restricted run could not reach those servers. Existing compiler warnings
  about float literals and unused code remain.
- Release executable compiled using the existing environment.
- Isolated installer: install, shortcuts, upgrade backup, uninstall and preserved
  sample project / downloads all passed with the existing Inno Setup compiler.
- Isolated updater: valid ZIP / installer staging, bad-checksum rejection, previous
  executable backup and mock restart all passed. Its helper now supports PowerShell 7.
- Integrity fixtures passed for manifest output, changed-file hash, duplicate names,
  and refusing unsigned executables when signed-only mode is required.
- Existing Desktop runtime: RTX 5070 Ti Laptop GPU, capability 12.0, PyTorch
  2.12.1+cu130, CUDA 13.0; small matrix kernel probe passed. No ComfyUI launch or
  model generation was performed during this validation.
- Installed stable app still reports v0.6.2. Its SHA-256 before and after validation:
  D5DD2CD448D842EF3667DB0438D2CA33506F672FA0BE06081F8ECB2A58DA8FF2.

Native display / monitor acceptance and model generation are still manual checks.
