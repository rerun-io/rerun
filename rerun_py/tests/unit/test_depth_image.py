from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun as rr
from rerun.components import DepthMeter
from rerun.datatypes import TensorBuffer, TensorData, TensorDataLike, TensorDimension

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20))


IMAGE_INPUTS: list[TensorDataLike] = [
    # Full explicit construction
    TensorData(
        shape=[
            TensorDimension(10, "height"),
            TensorDimension(20, "width"),
        ],
        buffer=TensorBuffer(RANDOM_IMAGE_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_IMAGE_SOURCE,
]

METER_INPUTS = [1000, DepthMeter(1000)]


def depth_image_expected() -> Any:
    return rr.DepthImage(data=RANDOM_IMAGE_SOURCE, meter=1000)


def test_image() -> None:
    expected = depth_image_expected()

    for img, meter in zip(IMAGE_INPUTS, METER_INPUTS):
        arch = rr.DepthImage(data=img, meter=meter)

        assert arch == expected


GOOD_IMAGE_INPUTS: list[TensorDataLike] = [
    # Mono
    rng.uniform(0.0, 1.0, (10, 20)),
    # Assorted Extra Dimensions
    rng.uniform(0.0, 1.0, (1, 10, 20)),
    rng.uniform(0.0, 1.0, (10, 20, 1)),
]

BAD_IMAGE_INPUTS: list[TensorDataLike] = [
    rng.uniform(0.0, 1.0, (10, 20, 3)),
    rng.uniform(0.0, 1.0, (10, 20, 4)),
    rng.uniform(0.0, 1.0, (10,)),
    rng.uniform(0.0, 1.0, (1, 10, 20, 3)),
    rng.uniform(0.0, 1.0, (1, 10, 20, 4)),
    rng.uniform(0.0, 1.0, (10, 20, 3, 1)),
    rng.uniform(0.0, 1.0, (10, 20, 4, 1)),
    rng.uniform(0.0, 1.0, (10, 20, 2)),
    rng.uniform(0.0, 1.0, (10, 20, 5)),
    rng.uniform(0.0, 1.0, (10, 20, 3, 2)),
]


def test_depth_image_shapes() -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    for img in GOOD_IMAGE_INPUTS:
        rr.DepthImage(img)

    for img in BAD_IMAGE_INPUTS:
        with pytest.raises(TypeError):
            rr.DepthImage(img)
