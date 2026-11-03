# Stage 3.3 — AI Photo Denoise (experimental)

PixelForge uses the genuine Waifu2x ncnn Vulkan neural-network denoiser for noise reduction **without changing image dimensions**. This is a separate engine from Real-ESRGAN upscaling.

## Windows setup

Obtain the Windows portable archive from the [official Waifu2x ncnn Vulkan releases](https://github.com/nihui/waifu2x-ncnn-vulkan/releases).

Place the following files relative to the PixelForge project:

```text
tools/
  waifu2x/
    waifu2x-ncnn-vulkan.exe
    models-cunet/
      noise0_model.param
      noise0_model.bin
      noise1_model.param
      noise1_model.bin
      noise2_model.param
      noise2_model.bin
      noise3_model.param
      noise3_model.bin
      ...other model files...
```

The complete upstream `models-cunet` folder is recommended. The executable and model folder must be shipped alongside the PixelForge executable in portable distributions, under `tools/waifu2x/`.

## Usage

Open a photo, choose **AI Tools → Restoration**, select noise reduction 0–3 and click **AI Denoise (Original Size)**. The existing AI preview, comparison, strength blend, save, Vulkan GPU selector and Cancel Upscale control are reused. Automatic prefers a discrete Vulkan GPU. Model level 2 is the initial default.

## Limitations

- The neural denoiser is currently experimental and must be installed separately; the repository does not bundle third-party binaries or model weights.
- Denoising is designed for image noise; it is not yet a general deblur or AI sharpening tool.
- Large images can consume significant RAM during decoding and preview creation.
- The AI comparison view calls the reference image the Lanczos baseline in some existing UI labels; for denoise the baseline is the adjusted original, with no resizing.
- Runtime and image quality need testing on representative photographs before release.
