from __future__ import annotations

from typing import cast

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
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], order="F"),
    rr_dt.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    rr_dt.Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9]),
    rr_dt.Mat3x3(rows=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    rr_dt.Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9]),
    rr_dt.Mat3x3(columns=[[1, 4, 7], [2, 5, 8], [3, 6, 9]]),
    rr_dt.Mat3x3(rr_dt.Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    rr_dt.Mat3x3(rr_dt.Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    rr_dt.Mat3x3(rows=rr_dt.Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    rr_dt.Mat3x3(rows=rr_dt.Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    rr_dt.Mat3x3(columns=rr_dt.Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    rr_dt.Mat3x3(columns=rr_dt.Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
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
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], order="F"),
    rr_dt.Mat4x4([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
    rr_dt.Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
    rr_dt.Mat4x4(rows=[[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]),
    rr_dt.Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16]),
    rr_dt.Mat4x4(columns=[[1, 5, 9, 13], [2, 6, 10, 14], [3, 7, 11, 15], [4, 8, 12, 16]]),
    rr_dt.Mat4x4(rr_dt.Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
    rr_dt.Mat4x4(rr_dt.Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    rr_dt.Mat4x4(rows=rr_dt.Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
    rr_dt.Mat4x4(rows=rr_dt.Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    rr_dt.Mat4x4(columns=rr_dt.Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    rr_dt.Mat4x4(columns=rr_dt.Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
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


def test_mat3x3array() -> None:
    assert rr_dt.Mat3x3Array.from_similar(cast(rr_dt.Mat3x3ArrayLike, MAT_3X3_INPUT)) == rr_dt.Mat3x3Array.from_similar(
        [[1, 2, 3, 4, 5, 6, 7, 8, 9]] * len(MAT_3X3_INPUT)
    )


# Tests the snippet that are embedded in the docs.
def test_mat3x3_doc_text() -> None:
    import rerun.experimental as rr

    np.testing.assert_array_equal(
        rr.dt.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns, np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32)
    )
    np.testing.assert_array_equal(
        rr.dt.Mat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
    )

    np.testing.assert_array_equal(
        rr.dt.Mat3x3(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.dt.Mat3x3(columns=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )


@pytest.mark.parametrize("data", MAT_4X4_INPUT)
def test_mat4x4(data: rr_dt.Mat4x4Like) -> None:
    m = rr_dt.Mat4x4(data)
    assert_correct_mat4x4(m)


def test_mat4x4array() -> None:
    assert rr_dt.Mat4x4Array.from_similar(cast(rr_dt.Mat4x4ArrayLike, MAT_4X4_INPUT)) == rr_dt.Mat4x4Array.from_similar(
        [[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]] * len(MAT_4X4_INPUT)
    )


# Tests the snippet that are embedded in the docs.
def test_mat4x4_doc_text() -> None:
    import rerun.experimental as rr

    np.testing.assert_array_equal(
        rr.dt.Mat4x4([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]).flat_columns,
        np.array([1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.dt.Mat4x4([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]).flat_columns,
        np.array([1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16], dtype=np.float32),
    )

    np.testing.assert_array_equal(
        rr.dt.Mat4x4(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.dt.Mat4x4(columns=[[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], dtype=np.float32),
    )
