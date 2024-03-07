from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import UInt64ArrayLike


class UInt64Ext:
    """Extension for [UInt64][rerun.datatypes.UInt64]."""

    @staticmethod
    def native_to_pa_array_override(data: UInt64ArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.uint64).flatten()
        return pa.array(array, type=data_type)
