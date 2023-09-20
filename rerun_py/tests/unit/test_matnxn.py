from __future__ import annotations

import numpy as np
import pytest
from rerun.experimental import dt as rr_dt

MAT_3X3_INPUT = [
    [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
    [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
    np.array([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9]),
]


def assert_correct_mat3x3(m: rr_dt.Mat3x3 | None) -> None:
    assert m is not None
    assert np.all(m.flat_columns == np.array([1.0, 4.0, 7.0, 2.0, 5.0, 8.0, 3.0, 6.0, 9.0]))
    assert m.flat_columns.dtype == np.float32


MAT_4X4_INPUT = [
    [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]],
    [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0], [9.0, 10.0, 11.0, 12.0], [13.0, 14.0, 15.0, 16.0]],
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
    np.array([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
]


def assert_correct_mat4x4(m: rr_dt.Mat4x4 | None) -> None:
    assert m is not None
    assert np.all(
        m.flat_columns
        == np.array([1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0, 3.0, 7.0, 11.0, 15.0, 4.0, 8.0, 12.0, 16.0])
    )
    assert m.flat_columns.dtype == np.float32


@pytest.mark.parametrize("data", MAT_3X3_INPUT)
def test_mat3x3(data: rr_dt.Mat3x3Like) -> None:
    m = rr_dt.Mat3x3(data)
    assert_correct_mat3x3(m)


@pytest.mark.parametrize("data", MAT_4X4_INPUT)
def test_mat4x4(data: rr_dt.Mat4x4Like) -> None:
    m = rr_dt.Mat4x4(data)
    assert_correct_mat4x4(m)
