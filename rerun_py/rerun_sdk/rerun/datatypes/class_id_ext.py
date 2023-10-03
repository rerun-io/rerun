from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ClassIdArrayLike


class ClassIdExt:
    """Extension for [ClassId][rerun.datatypes.ClassId]."""

    @staticmethod
    def native_to_pa_array_override(data: ClassIdArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.uint16).flatten()
        return pa.array(array, type=data_type)
