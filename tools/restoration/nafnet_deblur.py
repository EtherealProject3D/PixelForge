"""NAFNet ONNX neural deblur adapter for PixelForge.
Install: py -m pip install opencv-python numpy onnxruntime
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
        raise RuntimeError("Install Python dependencies: py -m pip install opencv-python numpy onnxruntime") from exc
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
    # OpenCV DNN cannot reliably execute this dynamic-shape NAFNet model.
    # ONNX Runtime supports its shape operators and uses CPU inference.
    session = ort.InferenceSession(args.model, providers=["CPUExecutionProvider"])
    inputs = session.get_inputs()
    if len(inputs) != 1:
        raise RuntimeError(f"Expected one NAFNet input, got {len(inputs)}")
    blob = cv2.dnn.blobFromImage(padded, 1.0 / 255.0, swapRB=True, crop=False)
    output = session.run(None, {inputs[0].name: blob})[0]
    if output.ndim != 4 or output.shape[0] != 1 or output.shape[1] != 3:
        raise RuntimeError(f"Unexpected NAFNet output shape: {output.shape}")
    rgb = np.transpose(output[0], (1, 2, 0))[:height, :width]
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
