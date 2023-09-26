from __future__ import annotations

import io
import tempfile

import rerun as rr
from PIL import Image


def test_image_encoded_png() -> None:
    _, file_path = tempfile.mkstemp(suffix=".png")

    image = Image.new("RGBA", (300, 200), color=(0, 0, 0, 0))
    image.save(file_path)

    img = rr.ImageEncoded(path=file_path)

    assert img.data.shape[0].size == 200
    assert img.data.shape[1].size == 300
    assert img.data.buffer.kind == "u8"


def test_image_encoded_jpg() -> None:
    _, file_path = tempfile.mkstemp(suffix=".jpg")

    image = Image.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(file_path)

    img = rr.ImageEncoded(path=file_path)

    assert img.data.shape[0].size == 200
    assert img.data.shape[1].size == 300
    assert img.data.buffer.kind == "jpeg"


def test_image_encoded_jpg_from_bytes() -> None:
    bin = io.BytesIO()

    image = Image.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(bin, format="jpeg")

    img = rr.ImageEncoded(contents=bin)

    assert img.data.shape[0].size == 200
    assert img.data.shape[1].size == 300
    assert img.data.buffer.kind == "jpeg"

    bin.seek(0)
    img = rr.ImageEncoded(contents=bin.read())

    assert img.data.shape[0].size == 200
    assert img.data.shape[1].size == 300
    assert img.data.buffer.kind == "jpeg"
