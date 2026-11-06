# Pixel Forge v0.6.4

Pixel Forge v0.6.4 fixes Windows fullscreen from a maximized window and adds model-specific prompt guides to Image Studio.

## Windows fullscreen

- F11 now fills the monitor when entering fullscreen from a maximized window, including the area previously occupied by the taskbar. Exiting returns to the previous window state.

## Image Studio

- Prompt Guide follows the selected generation model and includes a copyable example, negative-prompt guidance, and model documentation. Its top-left position is shared across models, and each guide remembers its own size.
- Use the per-model scratchpad to draft prompts, save named prompts for reuse, and transfer drafts to Initial Prompt.
- The CyberRealistic Pony guide includes clickable Danbooru tags, custom tag categories, and a link to an external translator. Tags follow click order and go to the open scratchpad or directly to Initial Prompt. In the scratchpad, drag tags to reorder them and right-click to remove them.
- New generations start at Medium and Square (1:1). Steps and CFG start at the model's recommended values and can be reset under the sliders. Automatic negative prompts are off by default.
- The sidebar makes model selection, generation settings, and Create New Image easier to find. Model Library starts collapsed, and text fields have a steady caret.

Light mode and custom palettes are deferred to a later version.
