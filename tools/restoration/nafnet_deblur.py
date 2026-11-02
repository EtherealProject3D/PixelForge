"""NAFNet ONNX neural deblur adapter for PixelForge.
Install: py -m pip install opencv-python numpy onnxruntime-gpu
Model: https://huggingface.co/opencv/deblurring_nafnet
"""
import argparse
from pathlib import Path
import sys

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--model", required=True)
    args = parser.parse_args()
    try:
        import cv2
        import numpy as np
        import onnxruntime as ort
    except ImportError as exc:
        raise RuntimeError("Install Python dependencies: py -m pip install opencv-python numpy onnxruntime-gpu") from exc
    if not Path(args.model).is_file():
        raise FileNotFoundError(f"NAFNet ONNX model missing: {args.model}")
    image = cv2.imread(args.input, cv2.IMREAD_UNCHANGED)
    if image is None:
        raise ValueError(f"Cannot read input: {args.input}")
    if image.ndim != 3 or image.shape[2] not in (3, 4):
        raise ValueError("Expected RGB/RGBA PNG input")
    alpha = image[:, :, 3].copy() if image.shape[2] == 4 else None
    bgr = image[:, :, :3]
    height, width = bgr.shape[:2]
    # NAFNet has multiple downsampling stages. Pad to a multiple of 32.
    bottom = (-height) % 32
    right = (-width) % 32
    padded = cv2.copyMakeBorder(bgr, 0, bottom, 0, right, cv2.BORDER_REFLECT_101 if min(height, width) > 32 else cv2.BORDER_REPLICATE)
    # Require CUDA. Never silently fall back to CPU: full-frame NAFNet can
    # exhaust host RAM and make the desktop unresponsive.
    ort.preload_dlls(directory="")
    if "CUDAExecutionProvider" not in ort.get_available_providers():
        raise RuntimeError("CUDA provider unavailable. Install onnxruntime-gpu and compatible CUDA/cuDNN libraries; CPU deblur is disabled.")
    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(args.model, sess_options=options,
                                   providers=["CUDAExecutionProvider"])
    if "CUDAExecutionProvider" not in session.get_providers():
        raise RuntimeError("CUDA provider could not initialize; refusing CPU fallback.")
    inputs = session.get_inputs()
    if len(inputs) != 1:
        raise RuntimeError(f"Expected one NAFNet input, got {len(inputs)}")
    # Conservative overlapping tiles prevent full-frame inference from exhausting VRAM.
    tile_size = 256
    overlap = 32
    stride = tile_size - 2 * overlap
    result = np.zeros((height, width, 3), dtype=np.uint8)
    for y in range(0, height, stride):
        for x in range(0, width, stride):
            y0 = max(0, y - overlap)
            x0 = max(0, x - overlap)
            y1 = min(height, y + stride + overlap)
            x1 = min(width, x + stride + overlap)
            tile = bgr[y0:y1, x0:x1]
            th, tw = tile.shape[:2]
            pad_h = (-th) % 32
            pad_w = (-tw) % 32
            tile = cv2.copyMakeBorder(tile, 0, pad_h, 0, pad_w, cv2.BORDER_REFLECT_101)
            blob = cv2.dnn.blobFromImage(tile, 1.0 / 255.0, swapRB=True, crop=False)
            output = session.run(None, {inputs[0].name: blob})[0]
            if output.ndim != 4 or output.shape[0] != 1 or output.shape[1] != 3:
                raise RuntimeError(f"Unexpected NAFNet output shape: {output.shape}")
            rgb_tile = np.transpose(output[0], (1, 2, 0))[:th, :tw]
            rgb_tile = (np.clip(rgb_tile, 0, 1) * 255.0).round().astype(np.uint8)
            cy0, cx0 = y - y0, x - x0
            cy1 = cy0 + min(stride, height - y)
            cx1 = cx0 + min(stride, width - x)
            result[y:y + cy1 - cy0, x:x + cx1 - cx0] = rgb_tile[cy0:cy1, cx0:cx1]
    rgb = result
    bgr_result = cv2.cvtColor((np.clip(rgb, 0, 1) * 255.0).round().astype(np.uint8), cv2.COLOR_RGB2BGR)
    if alpha is not None:
        bgr_result = np.dstack((bgr_result, alpha))
    if not cv2.imwrite(args.output, bgr_result):
        raise RuntimeError("Could not write deblurred PNG")

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"NAFNet deblur error: {exc}", file=sys.stderr)
        sys.exit(1)
