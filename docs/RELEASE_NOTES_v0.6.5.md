# PixelForge v0.6.5

PixelForge v0.6.5 adds prompt assistance to Image Studio, gives Model Library its own window, and improves local processing setup and diagnostics.

## Image Studio

- Quick Assist suggests a rule-based cleanup of your scratchpad draft. Review and edit the suggestion before applying it.
- Optional AI Assist uses an installed local Ollama text model to rewrite a draft for the selected image model. The scratchpad shows elapsed time and streams the suggestion as it is written.
- Scratchpad text editing includes spelling suggestions and a right-click clipboard menu.

## Dependencies and Model Library

- The Dependencies menu opens Setup and Maintenance or the dedicated Model Library. Generate also has a direct link to Model Library.
- Model Library groups AI Assist, Image Generation, Generation and Editing, Instruction Editing, and Restoration and Upscaling. Model cards show compatibility and memory notes.
- Setup and Maintenance can download and open the signed Ollama installer. Model Library can download Qwen2.5 3B through Ollama. Both downloads show progress.
- ComfyUI setup offers Auto, GPU, and CPU processing. If GPU startup fails, Generate offers a CPU retry. Diagnostics report Windows display adapters and preserve useful process errors. Image generation on the Radeon RX 5600M and performance in CPU mode have not been verified.

## Windows and appearance

- Floating windows remember their placement and resizable windows remember their size. Scroll handles remain visible while idle.
- PixelForge uses the new blue and teal icon for the app, installer shortcuts, and PixelForge project files associated with the app.
- The Dev launcher handles build exit codes more reliably, and the loading window opens without taking keyboard focus.
