#!/usr/bin/env python3
"""
Stream NV12 images from a webcam.

Run:
```sh
pip install -r examples/python/nv12/requirements.txt
python examples/python/nv12/main.py
```
"""
from __future__ import annotations

import argparse
import time

import cv2
import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk


def bgr2nv12(bgr: npt.NDArray[np.uint8]) -> npt.NDArray[np.uint8]:
    yuv: npt.NDArray[np.uint8] = cv2.cvtColor(bgr, cv2.COLOR_BGR2YUV_I420)
    uv_row_cnt = yuv.shape[0] // 3
    uv_plane = np.transpose(yuv[uv_row_cnt * 2 :].reshape(2, -1), [1, 0])
    yuv[uv_row_cnt * 2 :] = uv_plane.reshape(uv_row_cnt, -1)
    return yuv


def main() -> None:
    parser = argparse.ArgumentParser(description="Example of using the Rerun visualizer to display NV12 images.")
    rr.script_add_args(parser)
    parser.add_argument(
        "-t",
        "--timeout",
        type=float,
        default=5,
        help="Timeout in seconds, after which the script will stop streaming frames.",
    )
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_nv12")

    cap = cv2.VideoCapture(0)
    if not cap.isOpened():
        raise RuntimeError("This example requires a webcam.")
    start_time = time.time()
    print(f"Started streaming NV12 images for {args.timeout} seconds.")
    while start_time + args.timeout > time.time():
        ret, frame = cap.read()
        if not ret:
            time.sleep(0.01)
            continue
        rr.log(
            "NV12",
            rr.ImageEncoded(
                contents=bytes(bgr2nv12(frame)),
                format=rr.ImageFormat.NV12((frame.shape[0], frame.shape[1])),
            ),
        )
        time.sleep(0.01)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
