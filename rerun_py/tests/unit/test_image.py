from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20, 3))


IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
    # Full explicit construction
    rrd.TensorData(
        shape=[
            rrd.TensorDimension(10, "height"),
            rrd.TensorDimension(20, "width"),
            rrd.TensorDimension(3, "depth"),
        ],
        buffer=rrd.TensorBuffer(RANDOM_IMAGE_SOURCE),
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
    return rrc.TensorDataArray.from_similar(IMAGE_INPUTS[0])


def compare_images(left: Any, right: Any, check_fields: list[int]) -> None:
    for field in check_fields:
        assert left.storage.field(field) == right.storage.field(field)


def test_image() -> None:
    expected = tensor_data_expected()

    for input, check_fields in zip(IMAGE_INPUTS, CHECK_FIELDS):
        arch = rr2.Image(data=input)

        compare_images(arch.data, expected, check_fields)


GOOD_IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
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

BAD_IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
    rng.uniform(0.0, 1.0, (10,)),
    rng.uniform(0.0, 1.0, (10, 20, 2)),
    rng.uniform(0.0, 1.0, (10, 20, 5)),
    rng.uniform(0.0, 1.0, (10, 20, 3, 2)),
]


def test_image_shapes() -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    for img in GOOD_IMAGE_INPUTS:
        rr2.Image(img)

    for img in BAD_IMAGE_INPUTS:
        with pytest.raises(TypeError):
            rr2.Image(img)
