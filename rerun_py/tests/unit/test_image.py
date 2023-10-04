from __future__ import annotations

import io
from typing import Any

import numpy as np
import pytest
import rerun as rr
import torch
from PIL import Image as PILImage
from rerun.datatypes import TensorBuffer, TensorData, TensorDataLike, TensorDimension
from rerun.datatypes.tensor_data import TensorDataBatch
from rerun.error_utils import RerunWarning

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20, 3))


IMAGE_INPUTS: list[TensorDataLike] = [
    # Full explicit construction
    TensorData(
        shape=[
            TensorDimension(10, "height"),
            TensorDimension(20, "width"),
            TensorDimension(3, "depth"),
        ],
        buffer=TensorBuffer(RANDOM_IMAGE_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_IMAGE_SOURCE,
]

# 0 = shape
# 1 = buffer
CHECK_FIELDS: list[list[int]] = [
    [0, 1],
    [1],
]


def tensor_data_expected() -> Any:
    return TensorDataBatch(IMAGE_INPUTS[0])


def compare_images(left: Any, right: Any, check_fields: list[int]) -> None:
    for field in check_fields:
        assert left.as_arrow_array().storage.field(field) == right.as_arrow_array().storage.field(field)


def test_image() -> None:
    expected = tensor_data_expected()

    for input, check_fields in zip(IMAGE_INPUTS, CHECK_FIELDS):
        arch = rr.Image(data=input)

        compare_images(arch.data, expected, check_fields)


GOOD_IMAGE_INPUTS: list[TensorDataLike] = [
    # Mono
    rng.uniform(0.0, 1.0, (10, 20)),
    # RGB
    rng.uniform(0.0, 1.0, (10, 20, 3)),
    # RGBA
    rng.uniform(0.0, 1.0, (10, 20, 4)),
    # Assorted Extra Dimensions
    rng.uniform(0.0, 1.0, (1, 10, 20)),
    rng.uniform(0.0, 1.0, (1, 10, 20, 3)),
    rng.uniform(0.0, 1.0, (1, 10, 20, 4)),
    rng.uniform(0.0, 1.0, (10, 20, 1)),
    rng.uniform(0.0, 1.0, (10, 20, 3, 1)),
    rng.uniform(0.0, 1.0, (10, 20, 4, 1)),
    # Torch tensors
    torch.rand(10, 20, 1),
    torch.rand(10, 20, 3),
]

BAD_IMAGE_INPUTS: list[TensorDataLike] = [
    rng.uniform(0.0, 1.0, (10,)),
    rng.uniform(0.0, 1.0, (10, 20, 2)),
    rng.uniform(0.0, 1.0, (10, 20, 5)),
    rng.uniform(0.0, 1.0, (10, 20, 3, 2)),
]


def test_image_shapes() -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    for img in GOOD_IMAGE_INPUTS:
        rr.Image(img)

    for img in BAD_IMAGE_INPUTS:
        with pytest.raises(TypeError):
            rr.Image(img)


def test_image_compress() -> None:
    rr.set_strict_mode(False)

    # RGB Supported
    image_data = np.asarray(rng.uniform(0, 255, (10, 20, 3)), dtype=np.uint8)

    compressed = rr.Image(image_data).compress(jpeg_quality=80)
    assert type(compressed) == rr.ImageEncoded

    # Mono Supported
    image_data = np.asarray(rng.uniform(0, 255, (10, 20)), dtype=np.uint8)

    compressed = rr.Image(image_data).compress(jpeg_quality=80)
    assert type(compressed) == rr.ImageEncoded

    # RGBA Not supported
    with pytest.warns(RerunWarning) as warnings:
        image_data = np.asarray(rng.uniform(0, 255, (10, 20, 4)), dtype=np.uint8)
        compressed = rr.Image(data=TensorData(array=image_data)).compress(jpeg_quality=80)

        assert len(warnings) == 1
        assert "Only RGB or Mono images are supported for JPEG compression" in str(warnings[0])

        # Should still be an Image
        assert type(compressed) == rr.Image

    bin = io.BytesIO()
    image = PILImage.new("RGB", (300, 200), color=(0, 0, 0))
    image.save(bin, format="jpeg")

    # Jump through some hoops to make a pre-compressed image
    img_encoded = rr.ImageEncoded(contents=bin)
    img = rr.Image(img_encoded.data)

    with pytest.warns(RerunWarning) as warnings:
        img.compress()
        assert len(warnings) == 1
        assert "Image is already compressed as JPEG" in str(warnings[0])
