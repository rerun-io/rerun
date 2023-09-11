from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt


def override_tensor_buffer__inner_converter_override(inner: npt.ArrayLike) -> npt.NDArray[Any]:
    # A tensor buffer is always a flat array
    return np.asarray(inner).flatten()
