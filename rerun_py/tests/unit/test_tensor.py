from __future__ import annotations

from typing import Any

import numpy as np
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

rng = np.random.default_rng(12345)
RANDOM_TENSOR_SOURCE = rng.uniform(0.0, 1.0, (8, 6, 3, 5))


TENSOR_DATA_INPUTS: list[rrd.TensorDataArrayLike | None] = [
    # Full explicit construction
    rrd.TensorData(
        id=rrd.TensorId(uuid=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
        shape=[
            rrd.TensorDimension(8),
            rrd.TensorDimension(6),
            rrd.TensorDimension(3),
            rrd.TensorDimension(5),
        ],
        buffer=rrd.TensorBuffer(RANDOM_TENSOR_SOURCE),
    ),
    # Implicit construction from ndarray
    RANDOM_TENSOR_SOURCE,
]

CHECK_TENSOR_ID: list[bool] = [True, False]


def tensor_data_expected() -> Any:
    return rrc.TensorDataArray.from_similar(TENSOR_DATA_INPUTS[0])


def compare_tensors(left: Any, right: Any, check_id: bool) -> None:
    # Skip tensor_id
    if check_id:
        assert left.storage.field(0) == right.storage.field(0)
    assert left.storage.field(1) == right.storage.field(1)
    assert left.storage.field(2) == right.storage.field(2)


def test_tensor() -> None:
    expected = tensor_data_expected()

    for input, check_id in zip(TENSOR_DATA_INPUTS, CHECK_TENSOR_ID):
        arch = rr2.Tensor(data=input)

        compare_tensors(arch.data, expected, check_id)
