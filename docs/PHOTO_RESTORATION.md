# Photo cleanup

Open or import a photo, then choose **AI Tools > Photo Restoration**.
**AI Restore** uses FLUX Klein by default, with Qwen 2509 and 2511 alternatives.
Edit the supplied preservation-focused instructions and sampling steps, then choose
**Restore Photo**. It runs the existing local ComfyUI editing pipeline against the
viewed version, keeps source dimensions/orientation, and appends a new result without
changing the left comparison reference. Models must be installed from Model Library.
These controls are independent of the Generate prompts and model selection.
Compare faces carefully: generative restoration can invent plausible fine detail.

For targeted cleanup without instruction-based generation:
Choose a starting preset, adjust the strength for the next cleanup if needed,
and run one operation at a time:

1. **Denoise Photo** reduces grain with Waifu2x at the original size.
2. **Reduce Blur** uses NAFNet for photographic blur, or **Sharpen Details** uses
   Real-ESRGAN luminance detail while retaining the original colors.
3. Compare versions using the shared Compare slider. Enlarge last with Upscaling.

The left comparison dropdown is a persistent reference (the original by default).
The right dropdown follows the selected thumbnail and new operation results.
Changing the left reference keeps that choice for later operations and project saves.
**Rotate Left / Rotate Right** update the selected version in 90-degree steps without
adding a thumbnail. Adjustments, selections, and saved selections rotate with the
image. Subsequent operations use the rotated source. Other versions stay unchanged.
Double-click the image workspace to fit and center it, including in Compare mode
and across Edit, AI Tools, Generate, and Selection.

Presets set noise reduction and blend strength; they do not run a chain automatically.
Gentle starts at Light / 65%, Grainy Photo at Medium / 80%, Soft Photo at Very Light / 75%.
The next-cleanup strength is independent of the previous result's preview strength.
Every successful operation in the shared workspace adds a version, preserves source
dimensions and alpha, and leaves earlier versions available. Save the project to
retain that history. A failed/cancelled operation does not add a version.

Use **Check Restoration Dependencies** to install or check the existing engines.
The GPU and tile controls below apply to Waifu2x/Real-ESRGAN. NAFNet uses its separate
Python runtime; large photos may take time. These tools cannot recover missing real
details reliably. The AI Restore button processes the whole image; for selection-based
generative damage repair, use Apply Edit to Selection on Generate.
