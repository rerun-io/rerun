from __future__ import annotations

import itertools
from typing import Any

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components import DepthMeter, ImageFormat
from rerun.datatypes import ChannelDatatype, Float32Like

rng = np.random.default_rng(12345)
RANDOM_IMAGE_SOURCE = rng.uniform(0.0, 1.0, (10, 20))

IMAGE_INPUTS: list[Any] = [
    RANDOM_IMAGE_SOURCE,
    RANDOM_IMAGE_SOURCE,
]

METER_INPUTS: list[Float32Like] = [1000, DepthMeter(1000)]


def depth_image_expected() -> Any:
    return rr.DepthImage(RANDOM_IMAGE_SOURCE, meter=1000)


def test_depth_image() -> None:
    ranges = [None, [0.0, 1.0], (1000, 1000)]

    for img, meter, depth_range in itertools.zip_longest(IMAGE_INPUTS, METER_INPUTS, ranges):
        if img is None:
            img = IMAGE_INPUTS[0]

        print(
            f"rr.DepthImage(\n    {img}\n    meter={meter!r}\n    depth_range={depth_range!r}\n)",
        )
        arch = rr.DepthImage(img, meter=meter, depth_range=depth_range)

        assert arch.buffer == rr.components.ImageBufferBatch._converter(img.tobytes())
        assert arch.format == rr.components.ImageFormatBatch._converter(
            ImageFormat(
                width=img.shape[1],
                height=img.shape[0],
                channel_datatype=ChannelDatatype.from_np_dtype(img.dtype),
            ),
        )
        assert arch.meter == rr.components.DepthMeterBatch._converter(meter)
        assert arch.depth_range == rr.components.ValueRangeBatch._converter(depth_range)


GOOD_IMAGE_INPUTS: list[Any] = [
    # Mono
    rng.uniform(0.0, 1.0, (10, 20)),
    # Assorted Extra Dimensions
    rng.uniform(0.0, 1.0, (1, 10, 20)),
    rng.uniform(0.0, 1.0, (10, 20, 1)),
    torch.rand(10, 20, 1),
]

BAD_IMAGE_INPUTS: list[Any] = [
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
        with pytest.raises(ValueError):
            rr.DepthImage(img)
