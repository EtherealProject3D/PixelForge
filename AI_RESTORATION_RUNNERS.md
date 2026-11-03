# Neural sharpening and deblurring — Windows setup

## AI Sharpen — connected to Real-ESRGAN

PixelForge uses its existing verified Real-ESRGAN Vulkan `realesrgan-x4plus` model to enhance detail, then resizes the AI result back to the input dimensions with Lanczos3. This is **AI detail enhancement**, not a separately trained sharpening model. It may hallucinate texture. The existing Real-ESRGAN executable and model files in `tools/realesrgan` are required.

## AI Deblur — connected to OpenCV NAFNet ONNX

Model: [OpenCV's NAFNet deblurring ONNX model](https://huggingface.co/opencv/deblurring_nafnet), derived from [NAFNet](https://github.com/megvii-research/NAFNet). The OpenCV model page lists the files under MIT. Verify model and dependency licenses before redistribution.

**One-time installation in Windows PowerShell:**

```powershell
cd C:\PixelForge
py -3 -m pip install --upgrade opencv-python numpy
New-Item -ItemType Directory -Force -Path .\tools\restoration | Out-Null
Invoke-WebRequest -Uri "https://huggingface.co/opencv/deblurring_nafnet/resolve/main/deblurring_nafnet_2025may.onnx?download=true" -OutFile ".\tools\restoration\deblurring_nafnet_2025may.onnx"
```

The model is approximately 92 MB. Python 3 and the Windows `py` launcher must be installed. The tracked script `tools/restoration/nafnet_deblur.py` must exist from the Git branch.

Expected layout:

```text
tools/
  realesrgan/
    realesrgan-ncnn-vulkan.exe
    models/...
  restoration/
    nafnet_deblur.py
    deblurring_nafnet_2025may.onnx
```

Run `cargo run --release`, open an image, choose **AI Tools → Restoration → AI Deblur**.

**Important limitations:**

- NAFNet deblurring currently uses **OpenCV DNN CPU inference**. The Vulkan GPU selector applies to sharpening, **not deblurring**. Large photos may be slow and memory-intensive; start testing with a modest image (e.g., 1024×768) before 6000×4000.
- The adapter pads image dimensions to a multiple of 32, preserving original output dimensions and alpha channel.
- Cancellation kills the Python process. Inference progress is not available yet.
- Sharpening uses the existing Real-ESRGAN GPU pipeline and can require substantial time/VRAM for large images.
- This integration has not been compiled or validated on the user's Windows machine yet. Model and dependencies are not redistributed with PixelForge.
- Image quality varies by image and blur type; severely lost detail cannot reliably be recovered.
