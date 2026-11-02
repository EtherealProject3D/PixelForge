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
