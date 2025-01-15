"""Testing of the deprecated `ImageEncoded` helper."""

from __future__ import annotations

import io
import tempfile

import cv2
import numpy as np
import pytest
import rerun as rr  # pip install rerun-sdk
from PIL import Image


def test_image_encoded_png() -> None:
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tmp:
        file_path = tmp.name

        image = Image.new("RGBA", (300, 200), color=(0, 0, 0, 0))
        image.save(tmp)
        tmp.close()  # Close the file before opening it again in `ImageEncoded` (Windows can't handle another opening)

        with pytest.warns(DeprecationWarning) as warnings:
            img = rr.ImageEncoded(path=file_path)
            print([str(w.message) for w in warnings])
            assert len(warnings) == 1

        assert type(img) is rr.EncodedImage
        assert img.media_type is not None
        media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

        assert media_type_arrow == "image/png"


def test_image_encoded_jpg() -> None:
    with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp:
        file_path = tmp.name

        image = Image.new("RGB", (300, 200), color=(0, 0, 0))
        image.save(tmp)
        tmp.close()  # Close the file before opening it again in `ImageEncoded` (Windows can't handle another opening)

        with pytest.warns(DeprecationWarning) as warnings:
            img = rr.ImageEncoded(path=file_path)
            print([str(w.message) for w in warnings])
            assert len(warnings) == 1

        assert type(img) is rr.EncodedImage
        assert img.media_type is not None
        media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

        assert media_type_arrow == "image/jpeg"


def test_image_encoded_mono_jpg() -> None:
    with tempfile.NamedTemporaryFile(suffix=".jpeg", delete=False) as tmp:
        file_path = tmp.name

        image = Image.new("L", (300, 200), color=0)
        image.save(tmp)
        tmp.close()  # Close the file before opening it again in `ImageEncoded` (Windows can't handle another opening)

        with pytest.warns(DeprecationWarning) as warnings:
            img = rr.ImageEncoded(path=file_path)
            print([str(w.message) for w in warnings])
            assert len(warnings) == 1

        assert type(img) is rr.EncodedImage
        assert img.media_type is not None
        media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

        assert media_type_arrow == "image/jpeg"


def test_image_encoded_jpg_from_bytes() -> None:
    bin = io.BytesIO()

    image = Image.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(bin, format="jpeg")

    with pytest.warns(DeprecationWarning) as warnings:
        img = rr.ImageEncoded(contents=bin, format=rr.ImageFormat.JPEG)
        print([str(w.message) for w in warnings])
        assert len(warnings) == 1

    assert type(img) is rr.EncodedImage
    assert img.media_type is not None
    media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

    assert media_type_arrow == "image/jpeg"

    bin.seek(0)

    with pytest.warns(DeprecationWarning) as warnings:
        img = rr.ImageEncoded(contents=bin.read(), format=rr.ImageFormat.JPEG)
        assert len(warnings) == 1

    assert type(img) is rr.EncodedImage
    assert img.media_type is not None
    media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

    assert media_type_arrow == "image/jpeg"


def test_image_encoded_mono_jpg_from_bytes() -> None:
    bin = io.BytesIO()

    image = Image.new("L", (300, 200), color=0)
    image.save(bin, format="jpeg")

    with pytest.warns(DeprecationWarning) as warnings:
        img = rr.ImageEncoded(contents=bin, format=rr.ImageFormat.JPEG)
        print([str(w.message) for w in warnings])
        assert len(warnings) == 1

    assert type(img) is rr.EncodedImage
    assert img.media_type is not None
    media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

    assert media_type_arrow == "image/jpeg"

    bin.seek(0)

    with pytest.warns(DeprecationWarning) as warnings:
        img = rr.ImageEncoded(contents=bin.read(), format=rr.ImageFormat.JPEG)
        print([str(w.message) for w in warnings])
        assert len(warnings) == 1

    assert type(img) is rr.EncodedImage
    assert img.media_type is not None
    media_type_arrow = img.media_type.as_arrow_array()[0].as_py()

    assert media_type_arrow == "image/jpeg"


def test_image_encoded_nv12() -> None:
    def bgr2nv12(bgr: cv2.typing.MatLike) -> cv2.typing.MatLike:
        yuv = cv2.cvtColor(bgr, cv2.COLOR_BGR2YUV_I420)
        uv_row_cnt = yuv.shape[0] // 3
        uv_plane = np.transpose(yuv[uv_row_cnt * 2:].reshape(2, -1), [1, 0])
        yuv[uv_row_cnt * 2:] = uv_plane.reshape(uv_row_cnt, -1)
        return yuv

    img_bgr = np.random.randint(0, 255, (480, 640, 3), dtype=np.uint8)

    with pytest.warns(DeprecationWarning) as warnings:
        img = rr.ImageEncoded(
            contents=bytes(bgr2nv12(img_bgr)),
            format=rr.ImageFormat.NV12((480, 640)),
            draw_order=42,
        )
        print([str(w.message) for w in warnings])
        assert len(warnings) == 1

    assert type(img) is rr.Image
    assert img.format is not None
    
    image_format_arrow = img.format.as_arrow_array()[0].as_py()

    image_format = rr.components.ImageFormat(
        width=image_format_arrow["width"],
        height=image_format_arrow["height"],
        pixel_format=image_format_arrow["pixel_format"],
        channel_datatype=image_format_arrow["channel_datatype"],
        color_model=image_format_arrow["color_model"],
    )

    assert image_format.width == 640
    assert image_format.height == 480
    assert image_format.pixel_format == rr.PixelFormat.NV12

    assert img.draw_order is not None
    draw_order_arrow = img.draw_order.as_arrow_array()[0].as_py()
    assert draw_order_arrow == 42
