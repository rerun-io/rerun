"""Testing of the deprecated `ImageEncoded` helper."""

from __future__ import annotations

import io
import tempfile

import cv2
import numpy as np
import rerun as rr  # pip install rerun-sdk
from PIL import Image


def test_image_encoded_png() -> None:
    _, file_path = tempfile.mkstemp(suffix=".png")

    image = Image.new("RGBA", (300, 200), color=(0, 0, 0, 0))
    image.save(file_path)

    img = rr.ImageEncoded(path=file_path)

    assert img.media_type == "image/png"


def test_image_encoded_jpg() -> None:
    _, file_path = tempfile.mkstemp(suffix=".jpg")

    image = Image.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(file_path)

    img = rr.ImageEncoded(path=file_path)

    assert img.media_type == "image/jpeg"


def test_image_encoded_mono_jpg() -> None:
    _, file_path = tempfile.mkstemp(suffix=".jpg")

    image = Image.new("L", (300, 200), color=0)
    image.save(file_path)

    img = rr.ImageEncoded(path=file_path)

    assert img.media_type == "image/jpeg"


def test_image_encoded_jpg_from_bytes() -> None:
    bin = io.BytesIO()

    image = Image.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(bin, format="jpeg")

    img = rr.ImageEncoded(contents=bin)

    assert img.media_type == "image/jpeg"

    bin.seek(0)
    img = rr.ImageEncoded(contents=bin.read())

    assert img.media_type == "image/jpeg"


def test_image_encoded_mono_jpg_from_bytes() -> None:
    bin = io.BytesIO()

    image = Image.new("L", (300, 200), color=0)
    image.save(bin, format="jpeg")

    img = rr.ImageEncoded(contents=bin)

    assert img.media_type == "image/jpeg"

    bin.seek(0)
    img = rr.ImageEncoded(contents=bin.read())

    assert img.media_type == "image/jpeg"


def test_image_encoded_nv12() -> None:
    def bgr2nv12(bgr: cv2.typing.MatLike) -> cv2.typing.MatLike:
        yuv = cv2.cvtColor(bgr, cv2.COLOR_BGR2YUV_I420)
        uv_row_cnt = yuv.shape[0] // 3
        uv_plane = np.transpose(yuv[uv_row_cnt * 2 :].reshape(2, -1), [1, 0])
        yuv[uv_row_cnt * 2 :] = uv_plane.reshape(uv_row_cnt, -1)
        return yuv

    img_bgr = np.random.randint(0, 255, (480, 640, 3), dtype=np.uint8)

    img = (
        rr.ImageEncoded(
            contents=bytes(bgr2nv12(img_bgr)),
            format=rr.ImageFormat.NV12((480, 640)),
            draw_order=42,
        ),
    )

    assert img.resolution == rr.Resolution2D(640, 480)
    assert img.pixel_format == rr.PixelFormat.NV12
    assert img.draw_order == 42
