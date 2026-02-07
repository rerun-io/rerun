"""
Rerun only formally supports Numpy > 2.

However there is ongoing community support for partial coverage with Numpy 1.
These are utilities for that support.
"""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt

IS_NUMPY_2 = int(np.__version__.split(".")[0]) >= 2

if not IS_NUMPY_2:
    import warnings

    warnings.warn(
        "numpy 1 detected. Rerun has only been tested with numpy 2.",
        DeprecationWarning,
        stacklevel=2,
    )


def asarray(item: Any, dtype: npt.DTypeLike = None, copy: bool | None = None) -> np.ndarray:
    """A compatibility wrapper around `np.asarray`."""
    if IS_NUMPY_2:
        return np.asarray(item, dtype=dtype, copy=copy)
    if copy is not None:
        return np.array(item, dtype=dtype, copy=copy)
    return np.asarray(item, dtype=dtype)
