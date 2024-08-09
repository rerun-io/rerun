from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.archetypes.image import Image
from rerun.datatypes.tensor_data import TensorData
from rerun.error_utils import RerunWarning

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20, 3))


IMAGE_INPUTS: list[Any] = [
    {"image": RANDOM_IMAGE_SOURCE},
    {"image": RANDOM_IMAGE_SOURCE, "width": 20, "height": 10},
    {"image": RANDOM_IMAGE_SOURCE, "color_model": "RGB", "width": 20, "height": 10},
    {"image": RANDOM_IMAGE_SOURCE, "color_model": rr.datatypes.ColorModel.RGB, "width": 20, "height": 10},
    {
        "bytes": RANDOM_IMAGE_SOURCE.tobytes(),
        "color_model": "RGB",
        "datatype": "f64",
        "width": 20,
        "height": 10,
    },
    {
        "bytes": RANDOM_IMAGE_SOURCE.tobytes(),
        "color_model": "RGB",
        "datatype": rr.datatypes.ChannelDatatype.F64,
        "width": 20,
        "height": 10,
    },
    {
        "bytes": RANDOM_IMAGE_SOURCE.tobytes(),
        "color_model": "RGB",
        "datatype": np.float64,
        "width": 20,
        "height": 10,
    },
    # This was allowed in 0.17
    {"image": TensorData(array=RANDOM_IMAGE_SOURCE)},
]


def image_data_expected() -> Any:
    return Image(RANDOM_IMAGE_SOURCE, color_model="RGB", width=20, height=10)


def test_image() -> None:
    expected = image_data_expected()

    for input in IMAGE_INPUTS:
        arch = rr.Image(**input)

        assert arch.buffer == expected.buffer
        assert arch.format == expected.format


GOOD_IMAGE_INPUTS: list[Any] = [
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

BAD_IMAGE_INPUTS: list[Any] = [
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
        with pytest.raises(ValueError):
            rr.Image(img)


def test_image_compress() -> None:
    rr.set_strict_mode(False)

    # RGB Supported
    image_data = np.asarray(rng.uniform(0, 255, (10, 20, 3)), dtype=np.uint8)

    compressed = rr.Image(image_data).compress(jpeg_quality=80)
    assert type(compressed) is rr.EncodedImage

    # Mono Supported
    image_data = np.asarray(rng.uniform(0, 255, (10, 20)), dtype=np.uint8)

    compressed = rr.Image(image_data).compress(jpeg_quality=80)
    assert type(compressed) is rr.EncodedImage

    # RGBA Not supported
    with pytest.warns(RerunWarning) as warnings:
        image_data = np.asarray(rng.uniform(0, 255, (10, 20, 4)), dtype=np.uint8)
        compressed = rr.Image(image_data, "RGBA").compress(jpeg_quality=80)

        assert len(warnings) == 1
        assert "Cannot JPEG compress an image of type" in str(warnings[0])

        assert type(compressed) is rr.Image

    # 16-bit Not supported
    with pytest.warns(RerunWarning) as warnings:
        image_data = np.asarray(rng.uniform(0, 255, (10, 20, 3)), dtype=np.uint16)
        compressed = rr.Image(image_data).compress(jpeg_quality=80)

        assert len(warnings) == 1
        assert "Cannot JPEG compress an image of datatype" in str(warnings[0])

        assert type(compressed) is rr.Image

    # Floating point not supported
    with pytest.warns(RerunWarning) as warnings:
        image_data = np.asarray(rng.uniform(0, 255, (10, 20)), dtype=np.float32)
        compressed = rr.Image(image_data).compress(jpeg_quality=80)

        assert len(warnings) == 1
        assert "Cannot JPEG compress an image of datatype" in str(warnings[0])

        assert type(compressed) is rr.Image
