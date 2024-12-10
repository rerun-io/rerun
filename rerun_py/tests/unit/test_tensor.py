from __future__ import annotations

from typing import Any

import numpy as np
import pytest
import rerun as rr
from rerun.components import TensorData, TensorDataBatch
from rerun.datatypes import TensorBuffer, TensorDataLike

rng = np.random.default_rng(12345)
RANDOM_TENSOR_SOURCE = rng.uniform(0.0, 1.0, (8, 6, 3, 5))


TENSOR_DATA_INPUTS: list[TensorDataLike] = [
    # Full explicit construction
    TensorData(
        shape=[8, 6, 3, 5],
        dim_names=["a", "b", "c", "d"],
        buffer=TensorBuffer(RANDOM_TENSOR_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_TENSOR_SOURCE,
    # Explicit construction from array
    TensorData(array=RANDOM_TENSOR_SOURCE),
    # Explicit construction from array
    TensorData(array=RANDOM_TENSOR_SOURCE, dim_names=["a", "b", "c", "d"]),
    # Explicit construction from array
    TensorData(array=RANDOM_TENSOR_SOURCE, dim_names=["a", "b", "c", "d"]),
]

SHAPE = 0  # Based on datatypes/tensor_data.fbs
NAMES = 1  # Based on datatypes/tensor_data.fbs
BUFFER = 2  # Based on datatypes/tensor_data.fbs
CHECK_FIELDS: list[list[int]] = [
    [SHAPE, NAMES, BUFFER],
    [BUFFER],
    [BUFFER],
    [SHAPE, NAMES, BUFFER],
    [SHAPE, NAMES, BUFFER],
]


def tensor_data_expected() -> Any:
    return TensorDataBatch(TENSOR_DATA_INPUTS[0])


def compare_tensors(left: Any, right: Any, check_fields: list[int]) -> None:
    for field in check_fields:
        assert left.as_arrow_array().field(field) == right.as_arrow_array().field(field)


def test_tensor() -> None:
    expected = tensor_data_expected()

    for input, check_fields in zip(TENSOR_DATA_INPUTS, CHECK_FIELDS):
        arch = rr.Tensor(data=input)

        compare_tensors(arch.data, expected, check_fields)


def test_bad_tensors() -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    # No buffers
    with pytest.raises(ValueError):
        TensorData()

    # Buffer with no indication of shape
    with pytest.raises(ValueError):
        TensorData(
            buffer=RANDOM_TENSOR_SOURCE,
        )

    # Both array and buffer
    with pytest.raises(ValueError):
        TensorData(
            array=RANDOM_TENSOR_SOURCE,
            buffer=RANDOM_TENSOR_SOURCE,
        )

    # Wrong size buffer for dimensions
    with pytest.raises(ValueError):
        TensorData(
            shape=[1, 2, 3],
            dim_names=["a", "b", "c", "d"],
            buffer=RANDOM_TENSOR_SOURCE,
        )

    # TODO(jleibs) send_warning bottoms out in TypeError but these ought to be ValueErrors

    # Wrong number of names
    with pytest.raises(ValueError):
        TensorData(
            dim_names=["a", "b", "c"],
            array=RANDOM_TENSOR_SOURCE,
        )

    # Shape disagrees with array
    with pytest.raises(ValueError):
        TensorData(
            shape=[1, 2, 3],
            dim_names=["a", "b", "c", "d"],
            array=RANDOM_TENSOR_SOURCE,
        )
