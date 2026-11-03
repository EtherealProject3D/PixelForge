# Shared image workspace

PixelForge v0.5.2-shared uses the same selected image, preview canvas, zoom/pan,
comparison controls and bottom version thumbnails across Generate, Edit, AI Tools
and Selection. Changing tools changes only the right panel. There is no separate
Open in Editor handoff, temporary generated PNG, or AI result screen in this flow.

- Generate retains its initial/change prompt boxes and model choices.
- Edit applies global and masked local adjustments to the selected version.
- Selection paints and manages masks on that same pixel grid. Switching tools
  retains the mask and undo stack; changing thumbnails restores that image's mask
  and adjustment values (a new selection undo stack starts for that image).
- Select Area in the common toolbar opens the Selection tools. For SDXL masked
  refinement, enable Use painted selection for inpainting under Changes prompt.
  Qwen continues to use whole-image instruction editing.
- AI Tools receives the selected image including global/local adjustments. A
  successful result joins the common thumbnail history automatically. The source
  is pinned when the job starts. Further blend-strength choices can be kept as
  additional versions using Keep AI Blend as Version.
- Save New Version, beside the bottom thumbnails, creates a raster snapshot with a parent link, retaining
  the previous source and settings. This replaces the separate manual parameter
  history controls in the shared workspace.
- Export Image renders the viewed version with its adjustments. Save Project
  keeps original raster snapshots plus per-version global/local adjustments,
  active masks and named selections. Masks are run-length encoded and validated.
  Existing gallery files without this optional metadata still load. Older app
  builds ignore the new editor metadata; Save New Version or export a flat
  image before sharing edited results with those builds.

Save retains the existing project image/metadata size limits. Project-load errors
reject invalid mask data instead of silently dropping edits. Native AI outputs
that exceed available image/texture memory may fail to enter shared history;
existing AI result files and Save Result remain available in that case.

Import, Save Project and Export remain in the main menus; the canvas toolbar contains only image/view controls. Save New Version creates a snapshot in memory; File > Save Project writes the complete history to disk.

## Verification

68 automated tests passed on Windows, including shared tool/thumbnail continuity,
Studio save/reopen with masks and adjustments, named selection roundtrip, invalid
mask validation and document switching. The debug application was built and run.
A two-image fixture was opened: Edit, AI Tools and Selection kept the same canvas
and filmstrip; a brush stroke survived switching to AI Tools and back, and the
saved project contained its mask data. Debug and release compilation checks passed.
GPU generation, restoration/upscaling and AI blend output adapters still require
hands-on acceptance testing with real images and the installed engines. The
user's running release application was left open; no release executable was
replaced while it was in use.

## Run

Save your current work and close old PixelForge windows, then use your repository:

```powershell
git pull --ff-only origin feature/stage-1-conversational-editing
cargo test --locked
cargo run --locked --release
```

The title should read PixelForge v0.5.2-shared. Changes remain on the existing
feature branch / draft PR until acceptance testing is complete. The working
pre-continuity commit is b2a0d84 (prior rollback branch also remains available).
## Prompt spelling

Initial prompt, Changes prompt and additional negative terms now use background
Windows English (United States) spelling checks. Spell check is enabled by default
and can be toggled inside Generation settings. Misspelled words receive red underlines;
right-click a marked word to choose a suggestion. There is no separate Spelling button.
Nothing is corrected automatically. Ignore for this session suppresses that word
across the prompt fields until the app closes, without changing the Windows
personal dictionary. Common model/workflow terms such as Qwen, ComfyUI, SDXL, LoRA
and CFG are excluded from flags.

Checks wait for a short typing pause and run off the UI thread. Corrections apply
only to the selected occurrence and reject results from older text. Native UTF-16
positions are converted to valid UTF-8 boundaries, including emoji/accented text.
This is spelling assistance, not grammar checking or prompt rewriting. English
Windows language support is required; an unavailable-provider message is shown
rather than silently claiming that a prompt is correct. Extremely long prompts
(over 32,000 characters) are not checked, and up to 64 issues are returned per check.
Non-Windows builds show an unsupported-platform message. Toggle state and ignored
words are session-only.

Validation: 71 tests passed, including an actual Windows spell-check provider test,
Unicode positions and targeted/stale correction checks. Windows debug build passed.
The app was launched, a test typo was underlined, its suggestions were opened, and
one correction was applied while the rest of the prompt and Qwen term stayed intact.

## Prompt order and Qwen steps

Prompt fields are ordered Initial prompt, Negative prompt, Changes prompt. The
visible negative terms apply to both generation and refinement; edits no longer
silently restore older negative terms from the source image. SDXL can include style
exclusions; Qwen uses only the explicitly typed negative terms.

Qwen now has its own Steps slider (1–100, default 20) under the edit-model selector.
It is independent of SDXL generation steps, persists in Studio projects, and is
recorded in each edit's metadata/progress. Old projects default to 20. Reuse settings
restores a Qwen result's edit-step count without overwriting generation steps.
The graph passes this setting to KSampler instead of hard-coding 20.

The standard FP8 Qwen model and its vision/text encoder are large. On a 12 GB GPU,
model offloading and image/text encoding can dominate latency as well as sampling.
Lowering steps reduces sampling work, with a quality tradeoff, but does not remove
model-loading or encoding costs. The optional Lightning adapter is not installed
or enabled by this change. No new GPU benchmark was run while the user's GPU was
already busy; automated tests verify the chosen steps and negative conditioning,
and project save/reopen verifies the independent Qwen setting.
