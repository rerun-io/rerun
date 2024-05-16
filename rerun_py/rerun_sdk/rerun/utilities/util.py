"""Simple utilities to be used for Rerun demos."""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt


def bounce_lerp(
    a: float, b: float, t: float | np.ndarray[Any, np.dtype[Any]]
) -> float | np.ndarray[Any, np.dtype[Any]]:
    """
    A linear interpolator that bounces between `a` and `b` as `t` goes above `1.0`.

    Parameters
    ----------
    a:
        Start value (t == 0).
    b:
        End value (t == 1).
    t:
        Interpolation coefficient.

    """
    tf = t % 1
    if int(t) % 2 == 0:
        return (1.0 - tf) * a + tf * b
    else:
        return tf * a + (1.0 - tf) * b


def interleave(arr1: npt.NDArray[np.float64], arr2: npt.NDArray[np.float64]) -> npt.NDArray[np.float64]:
    """
    Interleaves two numpy arrays.

    Parameters
    ----------
    arr1:
        A numpy array of arbitrary shape and length.
    arr2:
        A numpy array with the same shape and length as `arr1`.

    """
    shape = list(arr1.shape)
    shape[0] *= 2
    arr = np.empty(shape, dtype=arr1.dtype)
    arr[0::2] = arr1
    arr[1::2] = arr2
    return arr
