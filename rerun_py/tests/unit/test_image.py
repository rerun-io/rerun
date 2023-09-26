from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun as rr
from rerun.datatypes import TensorBuffer, TensorData, TensorDataLike, TensorDimension
from rerun.datatypes.tensor_data import TensorDataBatch

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
