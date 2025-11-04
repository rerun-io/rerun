from __future__ import annotations

import numpy as np
import pytest
from rerun.datatypes import (
    QuaternionArrayLike,
    QuaternionBatch,
)

from .common_arrays import (
    quaternions_arrays,
    quaternions_expected,
)


@pytest.mark.parametrize("data", quaternions_arrays)
def test_quaternion_array_valid(data: QuaternionArrayLike) -> None:
    assert QuaternionBatch(data) == quaternions_expected(data, QuaternionBatch)


QUATERNION_INVALID_ARRAYS_INPUT = [
    [1],
    [1, 2],
    [1, 2, 3],
    # Single quaternions are via type checking encouraged to be constructed explicitly,
    # but we don't enforce that.
    # [1, 2, 3, 4],
    [1, 2, 3, 4, 5],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [[1], [2], [3], [4]],
    [[1, 2, 3, 4], [4, 5]],
    [[1, 2, 3, 4], [4, 5, 6, 7, 8]],
    [[1, 2, 3, 4, 5], [4, 5, 6, 7]],
]


@pytest.mark.parametrize("data", QUATERNION_INVALID_ARRAYS_INPUT)
def test_quaternion_array_invalid(data: QuaternionArrayLike) -> None:
    import rerun as rr

    rr.set_strict_mode(True)

    with pytest.raises(ValueError):
        QuaternionBatch(data)
    with pytest.raises(ValueError):
        QuaternionBatch(np.array(data))
