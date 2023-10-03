from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import DrawOrderArrayLike


class DrawOrderExt:
    """Extension for [DrawOrder][rerun.components.DrawOrder]."""

    @staticmethod
    def native_to_pa_array_override(data: DrawOrderArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)
