#!/usr/bin/env python3
"""Testing NV12 image encoding."""

from __future__ import annotations

import os
from argparse import Namespace
from typing import Any
from uuid import uuid4

import cv2
import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Chroma subsampling

All images should look roughly the same except for some chroma artifacts
and slight color differences due to different yuv conversion matrix coefficients.

Naturally, Y8 formats are greyscale.
"""


def bgra2y_u_v24(bgra: Any, full_range: bool) -> np.ndarray:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(u).flatten()
    v = np.array(v).flatten()
    yuv24 = np.concatenate((y, u, v))
    return yuv24.astype(np.uint8)


def bgra2y_u_v16(bgra: Any, full_range: bool) -> np.ndarray:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(cv2.resize(u, (u.shape[1] // 2, u.shape[0]))).flatten()
    v = np.array(cv2.resize(v, (v.shape[1] // 2, v.shape[0]))).flatten()
    yuv16 = np.concatenate((y, u, v))
    return yuv16.astype(np.uint8)


def bgra2y_u_v12(bgra: Any, full_range: bool) -> np.ndarray:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(cv2.resize(u, (u.shape[1] // 2, u.shape[0] // 2))).flatten()
    v = np.array(cv2.resize(v, (v.shape[1] // 2, v.shape[0] // 2))).flatten()
    yuv12 = np.concatenate((y, u, v))
    return yuv12.astype(np.uint8)


def bgra2y8(bgra: Any, full_range: bool) -> np.ndarray:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, _v, _u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, _u, _v = cv2.split(yuv)
    return y.astype(np.uint8)


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


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Grid(
        rrb.TextDocumentView(origin="readme"),
        rrb.Spatial2DView(origin="img_reference", name="Reference RGB"),
        rrb.Spatial2DView(origin="img_V_U_V24_limited_range", name="Y_U_V24_limited_range"),
        rrb.Spatial2DView(origin="img_V_U_V24_full_range", name="Y_U_V24_full_range"),
        rrb.Spatial2DView(origin="img_V_U_V16_limited_range", name="Y_U_V16_limited_range"),
        rrb.Spatial2DView(origin="img_V_U_V16_full_range", name="Y_U_V16_full_range"),
        rrb.Spatial2DView(origin="img_V_U_V12_limited_range", name="Y_U_V12_limited_range"),
        rrb.Spatial2DView(origin="img_V_U_V12_full_range", name="Y_U_V12_full_range"),
        rrb.Spatial2DView(origin="img_y8_limited_range", name="Y8_limited_range"),
        rrb.Spatial2DView(origin="img_y8_full_range", name="Y8_full_range"),
        rrb.Spatial2DView(origin="img_nv12", name="NV12"),
        rrb.Spatial2DView(origin="img_yuy2", name="YUY2"),
    )


def log_data() -> None:
    # Make sure you use a colorful image!
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = f"{dir_path}/../../../crates/viewer/re_ui/data/logo_dark_mode.png"
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)

    img_rgb = cv2.cvtColor(img_bgra, cv2.COLOR_BGRA2RGB)
    rr.log("img_reference", rr.Image(img_rgb, "rgb"))

    rr.log(
        "img_V_U_V24_limited_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V24_LimitedRange,
            bytes=bgra2y_u_v24(img_bgra, False).tobytes(),
        ),
    )
    rr.log(
        "img_V_U_V16_limited_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V16_LimitedRange,
            bytes=bgra2y_u_v16(img_bgra, False).tobytes(),
        ),
    )
    rr.log(
        "img_V_U_V12_limited_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V12_LimitedRange,
            bytes=bgra2y_u_v12(img_bgra, False).tobytes(),
        ),
    )
    rr.log(
        "img_y8_limited_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y8_LimitedRange,
            bytes=bgra2y8(img_bgra, False).tobytes(),
        ),
    )

    rr.log(
        "img_V_U_V24_full_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V24_FullRange,
            bytes=bgra2y_u_v24(img_bgra, True).tobytes(),
        ),
    )
    rr.log(
        "img_V_U_V16_full_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V16_FullRange,
            bytes=bgra2y_u_v16(img_bgra, True).tobytes(),
        ),
    )
    rr.log(
        "img_V_U_V12_full_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y_U_V12_FullRange,
            bytes=bgra2y_u_v12(img_bgra, True).tobytes(),
        ),
    )
    rr.log(
        "img_y8_full_range",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.Y8_FullRange,
            bytes=bgra2y8(img_bgra, True).tobytes(),
        ),
    )

    rr.log(
        "img_nv12",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.NV12,
            bytes=bgra2nv12(img_bgra).tobytes(),
        ),
    )
    rr.log(
        "img_yuy2",
        rr.Image(
            width=img_bgra.shape[1],
            height=img_bgra.shape[0],
            pixel_format=rr.PixelFormat.YUY2,
            bytes=bgra2yuy2(img_bgra).tobytes(),
        ),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_data()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
