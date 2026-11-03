Current update: see [SHARED_WORKSPACE.md](SHARED_WORKSPACE.md) for the unified Generate/Edit/AI Tools/Selection canvas, current v0.5.2-shared run instructions and validation.

# Stage 1: conversational image editing

Rollback branch (created on GitHub before code changes):
`rollback/pre-stage-1-2026-11-02`
Commit: `51b2f2b4ccde6dc050c000f4a0ccc5f110436bc7`

Implementation: `feature/stage-1-conversational-editing`
Base: `dev/stage-3-5-local-generation`

## Architecture and behavior

The right sidebar has separate Initial prompt and Changes prompt boxes. Create New
Image uses the initial prompt; Apply Edit uses the changes prompt and viewed image.
Thumbnails are below the canvas. The visible progress bar reports sampling steps,
batch position, percentage, elapsed time, and the active model, including while
preparing the backend. Technical settings remain collapsed.

The edit-model dropdown can follow the generation checkpoint or choose a different
installed SDXL checkpoint. The resolved name is displayed before editing, saved in
result metadata, and the independent selection is saved with the Studio project.
Older projects default to following the generation model. The selector now also exposes Qwen-Image-Edit 2509 (FP8), a separate instruction-edit pipeline. See QWEN_EDIT_SETUP.md and scripts/Install-QwenEdit.ps1 for the three model files and setup checks. Initial prompt text remains intact
when applying changes. Less common actions remain under More.

The first message creates one image; subsequent messages use the selected gallery
image as img2img input. Each successful result appends an immutable image/settings
snapshot with its parent index and editing prompt. Failed or cancelled requests
retain the draft and existing versions. Send is disabled during a request.

Conversation history provides direct navigation/revert and parent comparison.
Reverting selects a snapshot; sending another edit creates a branch without
removing later results. Existing gallery selection, import/export, metadata,
ordinary batch generation, refinement and painted inpainting remain available.
Explicit gallery removal still deletes a snapshot and repairs parent indices.

Save/open the Generation Studio project to preserve images, prompts, lineage,
selection and comparison state. Existing PXG1 files remain compatible; no format
migration is needed. The unsent composer draft is session-only.

SDXL refinement uses CLIP guidance and img2img. The separate Qwen Edit option uses image-conditioned instruction encoding instead. No conversational language-model interpreter is included. Prompt context accumulates from
the source version; long or contradictory instructions may reduce quality because
CLIP has limited context. Describe the desired visual result and adjust strength.
Whole-image editing may change other details. Use existing masked refinement for
localized changes. Imported images use the current selected checkpoint and are
snapped to supported dimensions when edited.

## Windows PowerShell

Run inside your existing PixelForge repository (save any local work first):

```powershell
git fetch origin
git switch feature/stage-1-conversational-editing
git pull --ff-only origin feature/stage-1-conversational-editing
cargo test --locked
cargo run --locked --release
```

If tests report temporary-directory access denied, use a writable temporary folder:

```powershell
New-Item -ItemType Directory -Force .test-temp | Out-Null
$env:TEMP = (Join-Path (Get-Location) '.test-temp')
$env:TMP = $env:TEMP
cargo test --locked
```

## Manual acceptance checks before merging

1. Connect/Refresh Models and select an installed checkpoint.
2. Send an initial scene description. Confirm v1 appears with seed/settings.
3. Send two desired-result edits; confirm v2 then v3 use the preceding image.
4. Select v1 in Conversation history / Revert and send a different edit.
   Confirm v2/v3 remain and the new version says from v1.
5. Compare parent, then compare arbitrary versions using Compare A/B.
6. Save/reopen a Studio project; verify images, prompts and parent links.
7. Test ordinary batch generation, imported image editing, painted inpainting,
   export metadata, cancellation and an unavailable backend followed by retry.
8. Check layout at your normal window size and display scaling.

Validation performed: Windows Rust debug build and 61 automated tests passed,
including dedicated Qwen graph/setup validation, edit-model routing and new prompt/metadata and branched gallery project roundtrip tests.
Cargo.lock was synchronized with the already-declared backend dependencies;
the previous lock omitted several required packages. Existing compiler warnings
remain. The v0.5.1-stage1 Windows build was launched and visually checked: Initial prompt and Changes prompt both visible in the right sidebar, Qwen available in the edit dropdown, collapsed technical settings and no misleading editor image status. The top controls occupy two compact rows when an image is selected. A two-image fixture was also opened to verify the populated canvas, compact toolbar and bottom thumbnails. Required Qwen nodes were checked against the installed ComfyUI object_info schema. Actual Qwen/GPU edits, model quality, VRAM use and release execution remain unverified. Keep the feature branch unmerged until
those manual checks pass.

## Continue development or return to the saved version

All implementation and this handoff live in GitHub and local files. Continue with
normal chat, GitHub and PowerShell using the branch above; no special runtime is
introduced for this workflow.

To inspect the saved baseline without discarding current work:

```powershell
git fetch origin
git switch -c recovery/pre-stage-1 origin/rollback/pre-stage-1-2026-11-02
```

## Shared comparison toolbar update

All four tool tabs share `Left: Version` / `Compare` / `Right: Version`
controls above the canvas. Enable Compare and drag the vertical divider to
reveal the chosen versions. The left dropdown also chooses the working version.
Use `+ Save New Version` to keep manual edits as a separate comparison target.
Thumbnail and dropdown selection retain the current zoom and pan; Fit resets them.
The old Before / After section has been removed from the Edit sidebar.

Validation: 72 automated tests pass, including simulated divider dragging and
camera retention across version/tool changes. Windows release build passes.
Native mouse behavior and appearance still need a manual check at normal display scaling.

## Toolbar, readability, and selection-edit update

- More and Reuse Generation Settings removed. Remove Version is a direct toolbar
  button; Image Details opens an on-demand top-toolbar popup.
- Right-click any version thumbnail for Remove Image Version. Removal preserves
  remaining selection/comparison references and marks the project unsaved.
- Body, button, small, and monospace text share a 16-point default.
- The negative prompt retains an always-visible automatic SDXL exclusions checkbox.
- Apply Edit changes the whole image; Apply Edit to Selection is beside it and
  requires a nonempty selection. SDXL uses masked inpainting. Qwen receives the
  intact source and its output is composited through the selection locally.
  Both paths preserve source dimensions and exact original pixels outside the mask.
- Validation: 75 tests pass and Windows debug build passes. Release executable
  replacement was blocked by the user's running app. Native file-picker automation
  did not complete the populated UI check; live GPU selection editing is unverified.

## Prompt hierarchy and persistent details update

The generation model dropdown now sits directly above the initial prompt.
The negative prompt section displays its automatic-exclusions checkbox and,
when enabled, the actual automatic SDXL terms above the additional prompt field.
Sidebar controls use 18-point text and section headings use 23-point text.
Image details and prompts remain visible in the existing bottom status area;
long lines are truncated with full text available on hover. The details popup
and View menu (including unused mode placeholders) were removed.
The filmstrip reserves space for thumbnail captions based on font/padding size.

Validation: 76 tests pass and Windows debug build passes. A rendered-layout
regression test verifies caption clipping, automatic-negative visibility,
and model selector placement above the initial prompt. No live GPU work needed.

## Compact sidebar and selection-edge update

Sidebar width is constrained to 290–360 points (320 default). Body/control text
is 16 points with 21-point headings; dropdown labels sit above constrained,
truncated controls. Dropdown arrows and their hit targets are larger throughout.
Long help text wraps within the sidebar.

An active-selection badge and overlay toggle remain in the shared toolbar.
The selection overlay renders in every tool tab, and on the left version when
comparing. The brush cursor remains limited to the Selection tab.

Selection edits now use one inward-feathered composite (12 source pixels,
reduced for small selections), rather than two separate composites. Fully
protected pixels remain exact, including alpha. This reduces hard seams but
cannot correct semantic mismatch between independently generated regions.

Validation: 78 tests pass and Windows debug build passes. Added regression tests
for overlay rendering across tabs/comparison and gradual blending at hard edges.
Live GPU image quality and native layout at user scaling still need acceptance.

## Remaining generation-sidebar overflow fix

Qwen steps, generation Steps/CFG, and Change strength now place their labels
above the slider. Slider widths are calculated from their current container,
including the indentation inside expanded Generation settings. Sidebar width
and existing font sizes are unchanged. Generation-settings rendering is factored
into one reusable function so its expanded content can be tested directly.

Validation: 79 tests pass and Windows debug build passes. A layout regression
renders Qwen plus expanded settings at 270, 300, and 330 logical pixels, verifies
settings were drawn, and rejects content extending beyond the right boundary.

## Unified generation/edit Model Library

Model Library now sits directly below the prompts, outside Generation settings.
It lists all five curated models (SDXL Base, RealVisXL, Juggernaut, CyberRealistic
Pony, and Qwen Edit), using one Download & Install flow, installed/partial status,
combined byte progress, cancellation, and generation/edit selection actions.
The separate Qwen setup hyperlink is removed.

The shared installer targets the active ComfyUI models root and routes each file
to checkpoints, diffusion_models, text_encoders, or vae. Qwen installs all three
required files, reuses valid existing files, rejects suspicious existing sizes,
checks disk space and response sizes, and never replaces existing destinations.
Failed/cancelled downloads clean the active .part; completed files remain.
Large bundle downloads no longer have the old two-minute whole-request limit.

Validation: 84 tests pass and Windows debug build passes. Local HTTP fixtures
exercise bundle installs, shared-file reuse, aggregate progress, cancellation,
partial transfer failure, and no-overwrite publication. Official Qwen/SDXL Base
URLs respond successfully; existing Qwen files match expected size ranges.
No duplicate 30 GB download or live GPU edit was run for this installer change.


## Dependency setup and model removal

The top Dependencies action highlights missing required components and opens a
setup/maintenance inventory. It installs managed side-by-side versions of ComfyUI
portable, Waifu2x, Real-ESRGAN, NAFNet, and an isolated Python deblur runtime, with
progress, cancellation, basic validation, and reversible activation. Existing
ComfyUI Desktop and system Python are left untouched. New managed installs live
under LOCALAPPDATA/PixelForge/deps to avoid Windows path-length problems.

Model Library now has Remove Model confirmations listing exact paths and bytes.
Qwen shared encoder/VAE files are kept by default; partial bundles can be cleaned
explicitly. Processing/downloads block removal and dependency switching.

Validation: 90 regular tests pass; an additional opt-in integration test passed
real official package installs and a tiny CPU deblur inside isolated fixtures.
Native Windows inventory detected all current dependencies, and Qwen's 20.43 GB
editor-only removal preview was inspected and cancelled without deleting files.
ComfyUI portable full installation / GPU generation remains unverified. GPU driver
updates and rollback remain vendor-managed. See docs/DEPENDENCIES.md for details.
