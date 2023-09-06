from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20))


IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
    # Full explicit construction
    rrd.TensorData(
        shape=[
            rrd.TensorDimension(10, "height"),
            rrd.TensorDimension(20, "width"),
        ],
        buffer=rrd.TensorBuffer(RANDOM_IMAGE_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_IMAGE_SOURCE,
]

METER_INPUTS = [1000, rr2.cmp.DepthMeter(1000)]


def depth_image_expected() -> Any:
    return rr2.DepthImage(data=RANDOM_IMAGE_SOURCE, meter=1000)


def test_image() -> None:
    expected = depth_image_expected()

    for img, meter in zip(IMAGE_INPUTS, METER_INPUTS):
        arch = rr2.DepthImage(data=img, meter=meter)

        assert arch == expected


GOOD_IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
    # Mono
    rng.uniform(0.0, 1.0, (10, 20)),
    # Assorted Extra Dimensions
    rng.uniform(0.0, 1.0, (1, 10, 20)),
    rng.uniform(0.0, 1.0, (10, 20, 1)),
]

BAD_IMAGE_INPUTS: list[rrd.TensorDataArrayLike] = [
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
        rr2.DepthImage(img)

    for img in BAD_IMAGE_INPUTS:
        with pytest.raises(TypeError):
            rr2.DepthImage(img)
