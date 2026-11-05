# PixelForge v0.6.3 — development, not approved for final release

- Settings > Display: native fullscreen, F11 toggle, persistent mode and
  normal window placement. Uses the current monitor rather than a fixed resolution.
  Fullscreen includes top-right minimize, restore-window, and close controls; close
  uses the same unsaved-work checks as the native window X.
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
2. Move and resize the window, select Settings > Display > Fullscreen. Test the
   top-right minimize, restore-window and close controls (cancel any unsaved-work
   prompt to keep editing). Confirm no
   title bar / taskbar, and full monitor coverage. F11 returns to the same placement.
3. Enter fullscreen again and use F11. Repeat from a maximized window. Close while
   fullscreen, reopen, then F11. Check that the earlier normal placement is restored.
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

## Drag-and-drop opening

Drop local PNG, JPEG, WebP, BMP, TIFF or .pxf project files anywhere in the app.
Multiple images / editor projects open in order through the existing loaders.
Generation Studio projects use their stored gallery, prompts and settings; replacing
an unsaved workspace offers Save Workspace & Open, Discard & Open, or Cancel.
Drops wait while generation, processing, exporting or setup is active. Unsupported
or unreadable files report an error; an unreadable image does not replace the active
document. Windows Explorer drag-and-drop still follows normal Windows permissions.

Validation: 117 Rust tests passed, one optional dependency test ignored. Simulated
native drop events cover multiple images, corrupt images, editor projects and
Generation Studio project opening with an unsaved-work guard. Manually test by
dragging an image and .pxf from Explorer onto the dev window, including fullscreen.

## Crop, resolution and workspace refinements

- Edit > Crop Image opens crop controls in the existing workspace. Drag any corner
  or edge to resize, drag inside the frame to reposition it, middle-drag to move the
  image beneath the frame, and scroll to zoom. Fit / + / minus also control the crop
  preview. The image remains large enough to cover the crop; no blank edges are added.
- Lock the current aspect ratio or select any of the same landscape / portrait presets
  offered by Generate. Unlocking allows a free shape. Apply Crop creates a new version
  at individual-pixel precision; Cancel Crop leaves the source unchanged. Adjustments,
  active selection and named selection masks are preserved and cropped consistently.
  Switching to another image cancels the transient crop preview.
- Resolution options are now Small, Medium, Standard, Large and Extra Large. Medium
  has a 1024-pixel long side. Dimension rounding offers 16, 32 and 64 pixels, defaulting
  to 32 for new sessions. The app retains its 256–4096 generation size limits.
- Medium at 32-pixel rounding: square 1024×1024, 4:3 1024×768, 3:2 1024×672,
  16:9 1024×576, 32:9 1024×288; portrait presets swap width / height.
- Original preset IDs are retained in saved projects. Projects without a rounding
  field continue using 64 pixels; newly saved files retain the chosen rounding.
  ComfyUI generation validation now accepts multiples of 16. The official FLUX.2
  and SD3 latent nodes expose 16-pixel dimension steps:
  https://github.com/Comfy-Org/ComfyUI/blob/master/comfy_extras/nodes_flux.py
  https://github.com/Comfy-Org/ComfyUI/blob/master/comfy_extras/nodes_sd3.py
- Thumbnail images grow from a 68-pixel to a 92-pixel maximum side, with consistent
  116×96 clickable cells and tighter caption / scrollbar spacing. Save New Version
  is now beside Remove Version in the workspace toolbar.

Validation: 127 tests passed, zero failures, one optional dependency test ignored.
Includes free / locked crop geometry, real pointer-event resizing / panning / zoom,
cropped pixel and mask preservation, project save / reopen, old preset compatibility,
Medium dimensions and fine-grid validation. The thumbnail / toolbar layout was also
visually checked in a temporary profile. Actual ComfyUI generation at the new sizes
is still a manual laptop check; no model generation or package installation was run.

Try: open an image, select Edit > Crop Image, drag a corner with lock off, choose
16:9 with lock on, then pan and zoom beneath the frame. Apply and compare with the
original version. Save and reopen a test .pxf. Under Generate > Generation settings,
choose Medium and verify output dimensions while changing ratio and rounding.

## Crop zoom modes and numeric sizes

The crop sidebar now has editable Crop Width and Crop Height pixel fields. Click a
number to type or drag it to adjust; typed values commit on Enter or focus change.
With aspect lock off they are independent. With lock on, the edited dimension drives
the other, rounded to one pixel. Sizes stay inside the source image and preserve the
crop center where possible; these fields do not upscale or resample the original.

Zoom mode offers:

- Zoom image and crop together (default): scrolling over the image and the + / minus
  buttons enlarge or shrink both together, without changing the selected source pixels
  or output dimensions. Middle-drag pans the whole preview.
- Zoom image inside crop: the frame stays fixed on screen while the image scales
  beneath it, changing the selected source pixels and output dimensions. Middle-drag
  repositions the image inside the frame; blank edges remain prevented.

Changing the zoom option alone does not move the frame or alter the crop. Handles
and numeric sizing remain available in both modes. Fit restores the preview camera.
Apply Crop still creates a new version and retains the original.

Validation: 130 Rust tests passed, zero failures, one optional dependency test ignored.
Includes exact numeric size application through the save/reopen path, source-edge
limits, locked ratios from either axis, both zoom modes and mode switching.

Try: in Edit > Crop Image, enter a width and height with lock off; enable lock and
change just one dimension. Zoom with each mode and watch the fields: together mode
keeps the numbers unchanged; image-inside mode changes them as you zoom. Apply and
verify the new version's dimensions while the original remains available.

## Edit sidebar organization

The Edit sidebar is ordered Histogram, Crop, Image Adjustments, then Image Analysis.
All four use matching blue-accent headings with a disclosure arrow immediately after
the title. Click a heading to collapse or expand its content; each starts expanded.
Auto Enhance is the first subgroup within Image Adjustments, followed by the reset /
adjustment-target controls and sliders. Light & Tone, Tonal Range, Color, and the
analysis groups are plain subheadings, without separate collapse controls.

The existing 130 regression tests pass (one optional dependency test ignored).
Check the four section headings in Edit, collapse and reopen Image Adjustments, and
confirm Auto Enhance and all slider groups reappear together.
