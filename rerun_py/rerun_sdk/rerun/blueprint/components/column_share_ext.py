from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ColumnShareArrayLike


class ColumnShareExt:
    """Extension for [ColumnShare][rerun.blueprint.components.ColumnShare]."""

    @staticmethod
    def native_to_pa_array_override(data: ColumnShareArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)
