from __future__ import annotations

import itertools
from typing import Any

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components import DepthMeter, ImageFormat
from rerun.datatypes import ChannelDatatype, Float32Like
from rerun.error_utils import RerunWarning

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


def _compressed_blob_size(encoded_depth: Any) -> int:
    """Extract the byte size of the PNG blob from an EncodedDepthImage."""
    return len(encoded_depth.blob.as_arrow_array()[0].as_py())


def test_depth_image_compress() -> None:
    rr.set_strict_mode(False)

    # U16 supported (most common depth format)
    depth_data = np.asarray(rng.uniform(0, 65535, (10, 20)), dtype=np.uint16)
    compressed = rr.DepthImage(depth_data, meter=1000).compress()
    assert type(compressed) is rr.EncodedDepthImage

    # U8 supported
    depth_data = np.asarray(rng.uniform(0, 255, (10, 20)), dtype=np.uint8)
    compressed = rr.DepthImage(depth_data).compress()
    assert type(compressed) is rr.EncodedDepthImage

    # F32 not supported
    with pytest.warns(RerunWarning) as warnings:
        depth_data = np.asarray(rng.uniform(0, 1, (10, 20)), dtype=np.float32)
        compressed = rr.DepthImage(depth_data).compress()

        assert len(warnings) == 1
        assert "Cannot PNG compress a depth image of datatype" in str(warnings[0])
        assert type(compressed) is rr.DepthImage

    # U32 not supported
    with pytest.warns(RerunWarning) as warnings:
        depth_data = np.asarray(rng.uniform(0, 65535, (10, 20)), dtype=np.uint32)
        compressed = rr.DepthImage(depth_data).compress()

        assert len(warnings) == 1
        assert "Cannot PNG compress a depth image of datatype" in str(warnings[0])
        assert type(compressed) is rr.DepthImage


def test_depth_image_compress_reduces_size() -> None:
    """Verify that PNG compression actually reduces data size for realistic depth images."""
    rr.set_strict_mode(True)

    # Smooth gradient (simulates a flat wall receding) — highly compressible
    rows, cols = 480, 640
    gradient_u16 = np.tile(np.linspace(500, 10000, cols, dtype=np.uint16), (rows, 1))
    raw_size = gradient_u16.nbytes
    compressed = rr.DepthImage(gradient_u16, meter=1000).compress()
    assert type(compressed) is rr.EncodedDepthImage
    compressed_size = _compressed_blob_size(compressed)
    assert compressed_size < raw_size, f"PNG should be smaller than raw for a gradient: {compressed_size} >= {raw_size}"

    # Constant depth (e.g. flat floor) — maximally compressible
    constant_u16 = np.full((rows, cols), 3000, dtype=np.uint16)
    raw_size = constant_u16.nbytes
    compressed = rr.DepthImage(constant_u16).compress()
    compressed_size = _compressed_blob_size(compressed)
    assert compressed_size < raw_size, (
        f"PNG should be smaller than raw for constant data: {compressed_size} >= {raw_size}"
    )
    # Constant data should compress very aggressively (>90% reduction)
    assert compressed_size < raw_size * 0.1, (
        f"Constant image should compress to <10% of raw: {compressed_size} vs {raw_size}"
    )

    # U8 gradient
    gradient_u8 = np.tile(np.linspace(0, 255, cols, dtype=np.uint8), (rows, 1))
    raw_size = gradient_u8.nbytes
    compressed = rr.DepthImage(gradient_u8).compress()
    compressed_size = _compressed_blob_size(compressed)
    assert compressed_size < raw_size, (
        f"PNG should be smaller than raw for U8 gradient: {compressed_size} >= {raw_size}"
    )

    # Stepped depth (simulates discrete depth planes) — should compress well
    stepped_u16 = np.zeros((rows, cols), dtype=np.uint16)
    for i in range(4):
        stepped_u16[i * (rows // 4) : (i + 1) * (rows // 4), :] = 1000 * (i + 1)
    raw_size = stepped_u16.nbytes
    compressed = rr.DepthImage(stepped_u16).compress()
    compressed_size = _compressed_blob_size(compressed)
    assert compressed_size < raw_size, (
        f"PNG should be smaller than raw for stepped data: {compressed_size} >= {raw_size}"
    )


def test_depth_image_compress_level() -> None:
    """Verify that compress_level parameter affects output size."""
    rr.set_strict_mode(True)

    rows, cols = 480, 640
    gradient_u16 = np.tile(np.linspace(500, 10000, cols, dtype=np.uint16), (rows, 1))

    size_level_0 = _compressed_blob_size(rr.DepthImage(gradient_u16).compress(compress_level=0))
    size_level_9 = _compressed_blob_size(rr.DepthImage(gradient_u16).compress(compress_level=9))

    assert size_level_9 < size_level_0, (
        f"Level 9 should produce smaller output than level 0: {size_level_9} >= {size_level_0}"
    )


def test_depth_image_compress_preserves_fields() -> None:
    rr.set_strict_mode(True)

    depth_data = np.asarray(rng.uniform(0, 65535, (10, 20)), dtype=np.uint16)
    original = rr.DepthImage(
        depth_data,
        meter=1000,
        depth_range=[100.0, 60000.0],
        point_fill_ratio=0.5,
        draw_order=1.0,
    )
    compressed = original.compress()

    assert type(compressed) is rr.EncodedDepthImage
    assert compressed.meter is not None
    assert compressed.depth_range is not None
    assert compressed.point_fill_ratio is not None
    assert compressed.draw_order is not None
    assert compressed.media_type is not None
