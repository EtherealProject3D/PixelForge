# AI Sharpen / Deblur integration (experimental)

PixelForge now has a same-resolution neural-restoration **integration layer**, not bundled sharpening/deblurring models.

## Executable contract

- `tools/restoration/sharpen.exe`: dedicated neural sharpening runner
- `tools/restoration/deblur.exe`: dedicated neural deblurring runner
- Invoked as `<exe> --input <absolute_input_png> --output <absolute_output_png>`
- Must produce a PNG with **exactly the same dimensions** as the input.
- A nonzero exit status reports stderr in the UI (last 16 KiB).
- Cancellation terminates the process.
- PixelForge stores original/restored full-resolution PNGs and shows an adjustable before/after comparison.

These filenames are **integration contracts**, not downloadable products. Do not rename a generic image tool or traditional filter as a neural runner. An actual, validated neural model and inference runtime must be adapted to this contract before the buttons can produce results. The app displays an explicit missing-engine error otherwise.

## Known limitations

- No neural sharpening or deblurring model weights are included.
- No supported GPU selection or tiling contract is established for these runners yet.
- Results cannot be tested end-to-end until real model adapters are available.
- AI strength blending is inherited from the existing preview and export pipeline.
- The code has not been compiled or validated on Windows in this branch.
