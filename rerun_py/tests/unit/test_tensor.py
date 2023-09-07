from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

rng = np.random.default_rng(12345)
RANDOM_TENSOR_SOURCE = rng.uniform(0.0, 1.0, (8, 6, 3, 5))


TENSOR_DATA_INPUTS: list[rrd.TensorDataLike] = [
    # Full explicit construction
    rrd.TensorData(
        shape=[
            rrd.TensorDimension(8, name="a"),
            rrd.TensorDimension(6, name="b"),
            rrd.TensorDimension(3, name="c"),
            rrd.TensorDimension(5, name="d"),
        ],
        buffer=rrd.TensorBuffer(RANDOM_TENSOR_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_TENSOR_SOURCE,
    # Explicit construction from array
    rrd.TensorData(array=RANDOM_TENSOR_SOURCE),
    # Explicit construction from array
    rrd.TensorData(array=RANDOM_TENSOR_SOURCE, names=["a", "b", "c", "d"]),
    # Explicit construction from array
    rrd.TensorData(array=RANDOM_TENSOR_SOURCE, names=["a", "b", "c", "d"]),
]

# 0 = shape
# 1 = buffer
CHECK_FIELDS: list[list[int]] = [
    [0, 1],
    [1],
    [1],
    [0, 1],
    [0, 1],
]


def tensor_data_expected() -> Any:
    return rrc.TensorDataArray.from_similar(TENSOR_DATA_INPUTS[0])


def compare_tensors(left: Any, right: Any, check_fields: list[int]) -> None:
    for field in check_fields:
        assert left.storage.field(field) == right.storage.field(field)


def test_tensor() -> None:
    expected = tensor_data_expected()

    for input, check_fields in zip(TENSOR_DATA_INPUTS, CHECK_FIELDS):
        arch = rr2.Tensor(data=input)

        compare_tensors(arch.data, expected, check_fields)


def test_bad_tensors() -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    # No buffers
    with pytest.raises(ValueError):
        rrd.TensorData(),

    # Buffer with no indication of shape
    with pytest.raises(ValueError):
        rrd.TensorData(
            buffer=RANDOM_TENSOR_SOURCE,
        ),

    # Both array and buffer
    with pytest.raises(ValueError):
        rrd.TensorData(
            array=RANDOM_TENSOR_SOURCE,
            buffer=RANDOM_TENSOR_SOURCE,
        ),

    # Wrong size buffer for dimensions
    with pytest.raises(ValueError):
        rrd.TensorData(
            shape=[
                rrd.TensorDimension(8, name="a"),
                rrd.TensorDimension(6, name="b"),
                rrd.TensorDimension(3, name="c"),
                rrd.TensorDimension(4, name="d"),
            ],
            buffer=RANDOM_TENSOR_SOURCE,
        ),

    # TODO(jleibs) send_warning bottoms out in TypeError but these ought to be ValueErrors

    # Wrong number of names
    with pytest.raises(TypeError):
        rrd.TensorData(
            names=["a", "b", "c"],
            array=RANDOM_TENSOR_SOURCE,
        ),

    # Shape disagrees with array
    with pytest.raises(TypeError):
        rrd.TensorData(
            shape=[
                rrd.TensorDimension(8, name="a"),
                rrd.TensorDimension(6, name="b"),
                rrd.TensorDimension(5, name="c"),
                rrd.TensorDimension(3, name="d"),
            ],
            array=RANDOM_TENSOR_SOURCE,
        ),
