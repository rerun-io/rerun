from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import BoolArrayLike


class VisibleExt:
    """Extension for [Bool][rerun.datatypes.Bool]."""

    @staticmethod
    def native_to_pa_array_override(data: BoolArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.bool_).flatten()
        return pa.array(array, type=data_type)
