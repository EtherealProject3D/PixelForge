# PixelForge v0.6.7

- Browse the current image's group with Left / Right or arrows that fade in near the viewer edges. The filmstrip scrolls smoothly to keep the selection visible.
- Open Full Screen Image from the viewer's lower-right corner or press Alt+Enter. Scroll to zoom, drag to pan, and double-click to fit. Alt+Enter or Esc restores the editor and its camera. Full-screen exit keeps the image visible while Windows restores the window.
- Hide or restore the thumbnail strip and sidebar with the divider handles. Hiding the sidebar gives both the canvas and filmstrip more width.
- Click a group header to choose where new images and edits go. Press Escape to clear that choice. Without a selected group, new imports and generations go to Ungrouped, and edits stay with their source image's group.
- Filled group indicators mean collapsed; outlines mean expanded. Group names retain their image counts. Right-click blank filmstrip space to create an empty group.
- Select multiple project files in Open Project to open them in separate tabs. Ctrl+S saves the active project. Delete removes the current image version while retaining normal Delete behavior in text editors.
- PixelForge starts installed Ollama and configured ComfyUI backends in the background or connects to existing servers. Generation model choices come from the connected backend, with bundle requirements checked before listing.
- Prompt Guide opens with its Scratchpad visible, restores its size and position, and recovers an off-screen window. Danbooru tag categories wrap to the available width, and saved user tags can be dragged between categories.

## Validation

Windows automated tests passed with 216 tests and two opt-in tests skipped. These include project isolation, saving and reopening, full-screen camera restoration, Delete behavior in text fields, backend model choices, and saving 80 user tags with a category move persisted to disk. Local startup checks reached both Ollama and ComfyUI.

The visual full-screen transition needs user observation across Windows display configurations. The opt-in GPU inpainting and live dependency download tests were not rerun for this release. Executables remain unsigned, with SHA-256 checksums included for the installer and portable ZIP.
