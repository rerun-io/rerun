from __future__ import annotations

import numpy as np
import pytest
import rerun as rr

MAT_3X3_INPUT = [
    [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
    [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
    np.array([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9]),
]


def assert_correct_mat3x3(m: rr.dt.Mat3x3 | None) -> None:
    assert m is not None
    assert np.all(m.columns[0].xyz == np.array([1.0, 4.0, 7.0]))
    assert m.columns[0].xyz.dtype == np.float32
    assert np.all(m.columns[1].xyz == np.array([2.0, 5.0, 8.0]))
    assert m.columns[1].xyz.dtype == np.float32
    assert np.all(m.columns[2].xyz == np.array([3.0, 6.0, 9.0]))
    assert m.columns[2].xyz.dtype == np.float32


MAT_4X4_INPUT = [
    [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]],
    [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0], [9.0, 10.0, 11.0, 12.0], [13.0, 14.0, 15.0, 16.0]],
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
    np.array([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
]


def assert_correct_mat4x4(m: rr.dt.Mat4x4 | None) -> None:
    assert m is not None
    assert np.all(m.columns[0].xyzw == np.array([1.0, 5.0, 9.0, 13.0]))
    assert m.columns[0].xyzw.dtype == np.float32
    assert np.all(m.columns[1].xyzw == np.array([2.0, 6.0, 10.0, 14.0]))
    assert m.columns[1].xyzw.dtype == np.float32
    assert np.all(m.columns[2].xyzw == np.array([3.0, 7.0, 11.0, 15.0]))
    assert m.columns[2].xyzw.dtype == np.float32
    assert np.all(m.columns[3].xyzw == np.array([4.0, 8.0, 12.0, 16.0]))
    assert m.columns[3].xyzw.dtype == np.float32


@pytest.mark.parametrize("data", MAT_3X3_INPUT)
def test_mat3x3(data: rr.dt.Mat3x3Like) -> None:
    m = rr.dt.Mat3x3(data)
    assert_correct_mat3x3(m)


@pytest.mark.parametrize("data", MAT_4X4_INPUT)
def test_mat4x4(data: rr.dt.Mat4x4Like) -> None:
    m = rr.dt.Mat4x4(data)
    assert_correct_mat4x4(m)
