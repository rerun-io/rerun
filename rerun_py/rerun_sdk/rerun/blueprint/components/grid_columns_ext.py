from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import GridColumnsArrayLike


class GridColumnsExt:
    """Extension for [GridColumns][rerun.blueprint.components.GridColumns]."""

    @staticmethod
    def native_to_pa_array_override(data: GridColumnsArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.uint32).flatten()
        return pa.array(array, type=data_type)
