"""Simple utilities to be used for Rerun demos."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import numpy as np


def bounce_lerp(
    a: float,
    b: float,
    t: float | np.ndarray[Any, np.dtype[Any]],
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
