#!/usr/bin/env python3
"""Testing NV12 image encoding."""
from __future__ import annotations

import argparse
import os
from typing import Any

import cv2
import numpy as np
import rerun as rr


def bgra2nv12(bgra: Any) -> np.ndarray:
    yuv = cv2.cvtColor(bgra, cv2.COLOR_BGRA2YUV_I420)
    uv_row_cnt = yuv.shape[0] // 3
    uv_plane = np.transpose(yuv[uv_row_cnt * 2 :].reshape(2, -1), [1, 0])
    yuv[uv_row_cnt * 2 :] = uv_plane.reshape(uv_row_cnt, -1)
    return yuv


def bgra2yuy2(bgra: Any) -> np.ndarray:
    yuv = cv2.cvtColor(bgra, cv2.COLOR_BGRA2YUV_YUY2)
    (y, uv) = cv2.split(yuv)

    yuy2 = np.empty((y.shape[0], y.shape[1] * 2), dtype=y.dtype)
    yuy2[:, 0::2] = y
    yuy2[:, 1::4] = uv[:, ::2]
    yuy2[:, 3::4] = uv[:, 1::2]

    return yuy2


def main() -> None:
    parser = argparse.ArgumentParser(description="Displaying chroma downsampled images.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_chroma_downsampled")

    # Make sure you use a colorful image!
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = f"{dir_path}/../../../crates/re_ui/data/logo_dark_mode.png"
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)

    img_rgb = cv2.cvtColor(img_bgra, cv2.COLOR_BGRA2RGB)
    rr.log("img_reference", rr.Image(img_rgb))

    rr.log(
        "img_nv12",
        rr.ImageEncoded(
            contents=bytes(bgra2nv12(img_bgra)),
            format=rr.ImageFormat.NV12((img_bgra.shape[0], img_bgra.shape[1])),
        ),
    )
    rr.log(
        "img_yuy2",
        rr.ImageEncoded(
            contents=bytes(bgra2yuy2(img_bgra)),
            format=rr.ImageFormat.YUY2((img_bgra.shape[0], img_bgra.shape[1])),
        ),
    )

    rr.log("expectation", rr.TextDocument("The images should look the same, except for some chroma artifacts."))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
