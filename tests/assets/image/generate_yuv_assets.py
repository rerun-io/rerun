#!/usr/bin/env python3
"""Generate YUV test assets for chroma subsampling snapshot tests.

Previously this was the release checklist `check_chroma_subsampling.py`.
The conversion functions are used to generate .bin assets for the Rust
snapshot tests in `re_view_spatial`.

Run with: pixi run uv run python3 tests/assets/image/generate_yuv_assets.py
"""

from __future__ import annotations

import os
from typing import cast

import cv2
import numpy as np
import numpy.typing as npt


def bgra2y_u_v24(bgra: npt.NDArray[np.uint8], full_range: bool) -> npt.NDArray[np.uint8]:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(u).flatten()
    v = np.array(v).flatten()
    yuv24: npt.NDArray[np.uint8] = np.concatenate((y, u, v))
    return yuv24.astype(np.uint8)


def bgra2y_u_v16(bgra: npt.NDArray[np.uint8], full_range: bool) -> npt.NDArray[np.uint8]:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(cv2.resize(u, (u.shape[1] // 2, u.shape[0]))).flatten()
    v = np.array(cv2.resize(v, (v.shape[1] // 2, v.shape[0]))).flatten()
    yuv16: npt.NDArray[np.uint8] = np.concatenate((y, u, v))
    return yuv16.astype(np.uint8)


def bgra2y_u_v12(bgra: npt.NDArray[np.uint8], full_range: bool) -> npt.NDArray[np.uint8]:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, v, u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, u, v = cv2.split(yuv)
    y = np.array(y).flatten()
    u = np.array(cv2.resize(u, (u.shape[1] // 2, u.shape[0] // 2))).flatten()
    v = np.array(cv2.resize(v, (v.shape[1] // 2, v.shape[0] // 2))).flatten()
    yuv12: npt.NDArray[np.uint8] = np.concatenate((y, u, v))
    return yuv12.astype(np.uint8)


def bgra2y8(bgra: npt.NDArray[np.uint8], full_range: bool) -> npt.NDArray[np.uint8]:
    if full_range:
        yvu = cv2.cvtColor(bgra, cv2.COLOR_BGR2YCrCb)
        y, _v, _u = cv2.split(yvu)
    else:
        yuv = cv2.cvtColor(bgra, cv2.COLOR_BGR2YUV)
        y, _u, _v = cv2.split(yuv)
    return cast("npt.NDArray[np.uint8]", y).astype(np.uint8)


def bgra2nv12(bgra: npt.NDArray[np.uint8]) -> npt.NDArray[np.uint8]:
    yuv: npt.NDArray[np.uint8] = cv2.cvtColor(bgra, cv2.COLOR_BGRA2YUV_I420).astype(np.uint8)
    uv_row_cnt = yuv.shape[0] // 3
    uv_plane = np.transpose(yuv[uv_row_cnt * 2 :].reshape(2, -1), [1, 0])
    yuv[uv_row_cnt * 2 :] = uv_plane.reshape(uv_row_cnt, -1)
    return yuv


def bgra2yuy2(bgra: npt.NDArray[np.uint8]) -> npt.NDArray[np.uint8]:
    yuv = cv2.cvtColor(bgra, cv2.COLOR_BGRA2YUV_YUY2)
    (y, uv) = cv2.split(yuv)

    yuy2: npt.NDArray[np.uint8] = np.empty((y.shape[0], y.shape[1] * 2), dtype=y.dtype)
    yuy2[:, 0::2] = y
    yuy2[:, 1::4] = uv[:, ::2]
    yuy2[:, 3::4] = uv[:, 1::2]

    return yuy2


def main() -> None:
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = os.path.join(dir_path, "../../../crates/viewer/re_ui/data/logo_dark_mode.png")
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED).astype(np.uint8)

    assets: dict[str, npt.NDArray[np.uint8]] = {
        "logo_dark_mode_y_u_v24_limited.bin": bgra2y_u_v24(img_bgra, False),
        "logo_dark_mode_y_u_v24_full.bin": bgra2y_u_v24(img_bgra, True),
        "logo_dark_mode_y_u_v16_limited.bin": bgra2y_u_v16(img_bgra, False),
        "logo_dark_mode_y_u_v16_full.bin": bgra2y_u_v16(img_bgra, True),
        "logo_dark_mode_y_u_v12_limited.bin": bgra2y_u_v12(img_bgra, False),
        "logo_dark_mode_y_u_v12_full.bin": bgra2y_u_v12(img_bgra, True),
        "logo_dark_mode_y8_limited.bin": bgra2y8(img_bgra, False),
        "logo_dark_mode_y8_full.bin": bgra2y8(img_bgra, True),
        "logo_dark_mode_nv12.bin": bgra2nv12(img_bgra),
        "logo_dark_mode_yuy2.bin": bgra2yuy2(img_bgra),
    }

    for name, data in assets.items():
        out_path = os.path.join(dir_path, name)
        data.tofile(out_path)
        print(f"Wrote {out_path} ({data.nbytes} bytes)")


if __name__ == "__main__":
    main()
