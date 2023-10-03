from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt


class TensorBufferExt:
    """Extension for [TensorBuffer][rerun.datatypes.TensorBuffer]."""

    @staticmethod
    def inner__field_converter_override(inner: npt.ArrayLike) -> npt.NDArray[Any]:
        # A tensor buffer is always a flat array
        return np.asarray(inner).flatten()
