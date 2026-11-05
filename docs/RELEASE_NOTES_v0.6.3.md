# Pixel Forge v0.6.3

Pixel Forge v0.6.3 adds crop editing, clearer generation controls, configurable hotkeys, and improved Windows window management and startup feedback.

## Image editing and workspace

- Crop in the workspace with adjustable corners, locked or unlocked aspect ratios, generator-matched ratio presets, numeric pixel dimensions, image movement, and separate crop-view/image zoom modes.
- Organize Edit into collapsible Histogram, Crop, Image Adjustments, and Image Analysis sections; auto enhancement is part of Image Adjustments.
- Larger version thumbnails, Save New Version beside Remove Version, and comparison dividers shown only when Compare is enabled.
- Open images and projects by drag-and-drop. The compact + menu offers New Blank Project, Open Image, and Open Project.
- Centered save-before-exit dialog content with unchanged button sizes.

## Generation and local AI

- Add Medium resolution between Small and Standard, targeting a 1024-pixel long side, with dimension rounding choices of 16, 32, or 64 pixels.
- Show model- and VRAM-aware resolution guidance with unrestricted override.
- Place Generation Settings under Initial Prompt, label Select Model, and give Model Library its own collapsible section.
- Retain AI Result Strength between operations.
- Expand GPU, VRAM, driver, CUDA, and backend information in Dependencies, with actionable CUDA error diagnostics.
- Select Auto, Modern, or Legacy backends using separate environments without replacing existing Python packages or ComfyUI dependencies.

## Windows, settings, and controls

- Share a custom top bar across windowed and genuine fullscreen modes, with a centered bold Pixel Forge title, version, and window controls. F11 toggles fullscreen; Escape does not exit it.
- Remember window placement, display position, and windowed/fullscreen mode. Include a Reset Window Size and Position recovery action under Settings > Visuals.
- Organize Settings into Visuals, Mouse Controls, and Hotkeys tabs.
- Offer Undo/Redo where supported, customizable standard editing shortcuts and F11, reset-to-default, text context menus, and grouped control-reference lists.
- Use Alt+wheel over sliders for single-unit adjustment, Ctrl+Alt for decimal precision, and Shift+Alt for five-unit steps. Direct numeric fields retain wheel editing without Alt. Ctrl+drag gives precision; decimal-capable fields display trailing .0 for whole values.
- Add a native loading indicator, protection against duplicate launches during initialization, and confirmation before opening another ready instance. Dev and release launches are separate.
- Check for published updates at startup when enabled; highlight Updates when a newer release is available. The checkbox is in Updates. Dev updates from source instead.

## Downloads and integrity

- Windows x64 installer and portable ZIP are accompanied by SHA256SUMS.txt and RELEASE-INTEGRITY.json.
- Builds are unsigned. Windows reputation warnings can still occur; no security protections are disabled or bypassed.
- The installer retains a previous-executable backup. Existing settings, projects, models, and ComfyUI environments are preserved; v0.6.2 remains available as a fallback release.

## Validation and known limits

- 147 automated tests pass; one opt-in real dependency download test remains ignored.
- GTX 1080 / GTX 10-series generation is **unverified pending hardware testing**. Detection and backend-selection logic have automated coverage; not all models or modern CUDA packages support those GPUs.
- Actual GPU generation depends on the installed model, driver, backend, and VRAM. Resolution guidance is heuristic.
- Multi-monitor dragging, mixed Windows scaling, and ultrawide behavior include fixes and automated geometry checks; behavior can vary by display setup. Use Settings > Visuals > Reset Window Size and Position if recovery is needed.
- Undo/Redo supports applicable text and selection-mask history; general image-edit history is not promised.
- Light, Windows-following, and custom theme presets remain planned for v0.6.4.
