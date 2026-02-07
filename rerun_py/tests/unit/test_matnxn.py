from __future__ import annotations

from typing import cast

import numpy as np
import pytest
import torch
from rerun.datatypes import (
    Mat3x3,
    Mat3x3ArrayLike,
    Mat3x3Batch,
    Mat3x3Like,
    Mat4x4,
    Mat4x4ArrayLike,
    Mat4x4Batch,
    Mat4x4Like,
)

MAT_3X3_INPUT = [
    [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
    [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
    np.array([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    torch.tensor([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    torch.tensor(np.array([1, 2, 3, 4, 5, 6, 7, 8, 9])),
    np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], order="F"),
    Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9]),
    Mat3x3(rows=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9]),
    Mat3x3(columns=[[1, 4, 7], [2, 5, 8], [3, 6, 9]]),
    Mat3x3(Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    Mat3x3(Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    Mat3x3(rows=Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    Mat3x3(rows=Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    Mat3x3(columns=Mat3x3(columns=[1, 4, 7, 2, 5, 8, 3, 6, 9])),
    Mat3x3(columns=Mat3x3(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
]


def assert_correct_mat3x3(m: Mat3x3 | None) -> None:
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
    torch.tensor([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]),
    torch.tensor([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
    Mat4x4([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
    Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
    Mat4x4(rows=[[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]),
    Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16]),
    Mat4x4(columns=[[1, 5, 9, 13], [2, 6, 10, 14], [3, 7, 11, 15], [4, 8, 12, 16]]),
    Mat4x4(Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
    Mat4x4(Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    Mat4x4(rows=Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
    Mat4x4(rows=Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    Mat4x4(columns=Mat4x4(columns=[1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16])),
    Mat4x4(columns=Mat4x4(rows=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
]


def assert_correct_mat4x4(m: Mat4x4 | None) -> None:
    assert m is not None
    assert np.all(
        m.flat_columns
        == np.array([1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0, 3.0, 7.0, 11.0, 15.0, 4.0, 8.0, 12.0, 16.0]),
    )
    assert m.flat_columns.dtype == np.float32


@pytest.mark.parametrize("data", MAT_3X3_INPUT)
def test_mat3x3(data: Mat3x3Like) -> None:
    m = Mat3x3(data)
    assert_correct_mat3x3(m)


def test_mat3x3array() -> None:
    assert Mat3x3Batch(cast("Mat3x3ArrayLike", MAT_3X3_INPUT)) == Mat3x3Batch(
        [[1, 2, 3, 4, 5, 6, 7, 8, 9]] * len(MAT_3X3_INPUT),
    )


# Tests the snippet that are embedded in the docs.
def test_mat3x3_doc_text() -> None:
    np.testing.assert_array_equal(
        Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
        np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        Mat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
    )

    np.testing.assert_array_equal(
        Mat3x3(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        Mat3x3(columns=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )


@pytest.mark.parametrize("data", MAT_4X4_INPUT)
def test_mat4x4(data: Mat4x4Like) -> None:
    m = Mat4x4(data)
    assert_correct_mat4x4(m)


def test_mat4x4array() -> None:
    assert Mat4x4Batch(cast("Mat4x4ArrayLike", MAT_4X4_INPUT)) == Mat4x4Batch(
        [[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]] * len(MAT_4X4_INPUT),
    )


# Tests the snippet that are embedded in the docs.
def test_mat4x4_doc_text() -> None:
    np.testing.assert_array_equal(
        Mat4x4([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]).flat_columns,
        np.array([1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        Mat4x4([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]).flat_columns,
        np.array([1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16], dtype=np.float32),
    )

    np.testing.assert_array_equal(
        Mat4x4(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        Mat4x4(columns=[[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], dtype=np.float32),
    )
