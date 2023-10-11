from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ClearIsRecursiveArrayLike


class ClearIsRecursiveExt:
    """Extension for [ClearIsRecursive][rerun.components.ClearIsRecursive]."""

    @staticmethod
    def native_to_pa_array_override(data: ClearIsRecursiveArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.bool_).flatten()
        return pa.array(array, type=data_type)
