"""PixelForge NAFNet deblur using ONNX Runtime CPU.

The official quantized ONNX model is not compatible with OpenCV DNN's
dynamic-shape importer or the CUDA DequantizeLinear implementation.
Bounded, overlapping CPU tiles prevent whole-image inference memory spikes.
Install: py -3 -m pip install onnxruntime numpy opencv-python
"""
import argparse
import os
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--model", required=True)
    args = parser.parse_args()

    # Limit native library worker pools before importing inference libraries.
    os.environ["OMP_NUM_THREADS"] = "2"
    os.environ["OPENBLAS_NUM_THREADS"] = "2"
    import cv2
    import numpy as np
    import onnxruntime as ort

    if not Path(args.model).is_file():
        raise FileNotFoundError(f"NAFNet ONNX model missing: {args.model}")
    image = cv2.imread(args.input, cv2.IMREAD_UNCHANGED)
    if image is None or image.ndim != 3 or image.shape[2] not in (3, 4):
        raise ValueError("Expected RGB/RGBA PNG input")
    alpha = image[:, :, 3].copy() if image.shape[2] == 4 else None
    bgr = image[:, :, :3]
    height, width = bgr.shape[:2]

    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    session = ort.InferenceSession(
        args.model, sess_options=options, providers=["CPUExecutionProvider"]
    )
    if session.get_providers() != ["CPUExecutionProvider"]:
        raise RuntimeError("CPU inference provider unavailable")

    input_name = session.get_inputs()[0].name
    output_name = session.get_outputs()[0].name

    # Each inference is 256x256, divisible by NAFNet's 32-pixel stride.
    # 32-pixel context on every side limits seams at tile boundaries.
    tile = 256
    context = 32
    step = tile - 2 * context
    result = np.empty((height, width, 3), dtype=np.uint8)
    for y in range(0, height, step):
        for x in range(0, width, step):
            core_h = min(step, height - y)
            core_w = min(step, width - x)
            # Replicate edges, so even tiny images are valid inputs.
            patch = bgr[
                max(0, y - context):min(height, y + core_h + context),
                max(0, x - context):min(width, x + core_w + context),
            ]
            top = max(0, context - y)
            left = max(0, context - x)
            bottom = tile - patch.shape[0] - top
            right = tile - patch.shape[1] - left
            if min(top, left, bottom, right) < 0:
                raise RuntimeError("Invalid tile padding")
            patch = cv2.copyMakeBorder(
                patch, top, bottom, left, right, cv2.BORDER_REPLICATE
            )
            rgb = cv2.cvtColor(patch, cv2.COLOR_BGR2RGB)
            tensor = np.ascontiguousarray(
                rgb.transpose(2, 0, 1)[None].astype(np.float32) / 255.0
            )
            predicted = session.run([output_name], {input_name: tensor})[0]
            if predicted.shape != (1, 3, tile, tile):
                raise RuntimeError(f"Unexpected NAFNet output shape: {predicted.shape}")
            rgb_out = np.clip(
                predicted[0].transpose(1, 2, 0), 0, 1
            )
            core = (rgb_out[context:context + core_h,
                            context:context + core_w] * 255.0).round().astype(np.uint8)
            result[y:y + core_h, x:x + core_w] = cv2.cvtColor(
                core, cv2.COLOR_RGB2BGR
            )
    if alpha is not None:
        result = np.dstack((result, alpha))
    if not cv2.imwrite(args.output, result):
        raise RuntimeError("Could not write deblurred PNG")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"NAFNet deblur error: {exc}", file=sys.stderr)
        sys.exit(1)
