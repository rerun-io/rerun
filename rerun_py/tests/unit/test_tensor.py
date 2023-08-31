from __future__ import annotations

import itertools
from typing import Any, Optional, cast

import numpy as np
import pytest
import rerun.experimental as rr2
from numpy.random import default_rng
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd


rng = default_rng(12345)
tensor_data = rng.uniform(0.0, 1.0, (8, 6, 3, 5))


def tensor_data_expected() -> Any:
    return rrc.TensorDataArray.from_similar(tensor_data)


rng = default_rng(12345)
tensor_data_array: list[rrd.TensorDataArrayLike | None] = [tensor_data]


def compare_tensors(left, right):
    assert left.storage.field(1) == right.storage.field(1)
    assert left.storage.field(2) == right.storage.field(2)


def test_tensor() -> None:
    for data in tensor_data_array:
        arch = rr2.Tensor(
            data,
        )

        expected = tensor_data_expected()

        compare_tensors(arch.data, expected)
