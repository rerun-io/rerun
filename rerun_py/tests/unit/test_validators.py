from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
import pytest
from rerun._validators import flat_np_array_from_array_like

VALID_CASES = [
    # 1D array: length is a multiple of dimension
    (np.array([1, 2, 3, 4, 5, 6]), 3),
    (np.array([10, 20, 30, 40]), 2),
    # 2D array: shape is (n, dimension)
    (np.array([[1, 2, 3], [4, 5, 6]]), 3),
    # 3D array with extra singleton dimensions: last non-singleton dimension is dimension
    (np.array([[[1, 2, 3]], [[4, 5, 6]]]), 3),
    # Edge case: multi-dimensional array where all non-first dimensions are singletons (defaults to 1)
    (np.array([[[1]]]), 1),
    # Array of shape (1, 1, 1, 3)
    (np.array([[[[1, 2, 3]]]]), 3),
    # Array of shape (1, 1, 2, 3)
    (np.array([[[[1, 2, 3], [4, 5, 6]]]]), 3),
    # Array of shape (2, 2, 3)
    (
        np.array([[[1, 2, 3], [4, 5, 6]], [[7, 8, 9], [10, 11, 12]]]),
        3,
    ),
    # Array of shape (5, 1, 1, 3)
    (np.arange(15).reshape(5, 1, 1, 3), 3),
    # Array of shape (2, 1, 2) where the last non-singleton dimension is 2
    (np.array([[[1, 2]], [[3, 4]]]), 2),
]


@pytest.mark.parametrize(["input_array", "dimension"], VALID_CASES)
def test_flat_np_array_from_array_like_valid(input_array: npt.NDArray[Any], dimension: int) -> None:
    np.testing.assert_array_equal(flat_np_array_from_array_like(input_array, dimension), input_array.reshape(-1))


INVALID_CASES = [
    # 1D array: length not a multiple of dimension
    (np.array([1, 2, 3, 4]), 3),
    (np.array([1, 2, 3]), 2),
    # 2D array: shape is (n, m) where m != dimension
    (np.array([[1, 2], [3, 4]]), 3),
    # 3D array: last non-singleton dimension is not equal to dimension
    (np.array([[[1, 2]], [[3, 4]]]), 3),
    # Edge case: multi-dimensional array where all non-first dimensions are singletons (defaults to 1) but dimension is not 1
    (np.array([[[1]]]), 2),
    # 3D array with shape (2, 2, 2): last non-singleton dimension is 2, not equal to expected 3
    (np.array([[[1, 2], [3, 4]], [[5, 6], [7, 8]]]), 3),
    # 2D array with shape (3, 4): last dimension is 4, expected 3
    (np.array([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]]), 3),
    # 3D array with shape (1, 1, 4): last non-singleton dimension is 4, expected 3
    (np.array([[[1, 2, 3, 4]]]), 3),
    # 4D array with shape (1, 2, 1, 3): last non-singleton dimension is 3, expected 2
    (np.array([[[[1, 2, 3]], [[4, 5, 6]]]]), 2),
]


@pytest.mark.parametrize(["input_array", "dimension"], INVALID_CASES)
def test_flat_np_array_from_array_like_invalid(input_array: npt.NDArray[Any], dimension: int) -> None:
    with pytest.raises(ValueError):
        flat_np_array_from_array_like(input_array, dimension)
