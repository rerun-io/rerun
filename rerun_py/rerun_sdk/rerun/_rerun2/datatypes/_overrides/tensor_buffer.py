from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt


def tensorbuffer_inner_converter(inner: npt.ArrayLike) -> npt.NDArray[Any]:
    # A tensor buffer is always a flat array
    return np.asarray(inner).flatten()
