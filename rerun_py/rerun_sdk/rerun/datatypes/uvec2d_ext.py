from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .._validators import flat_np_uint32_array_from_array_like

if TYPE_CHECKING:
    from . import UVec2DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class UVec2DExt:
    """Extension for [UVec2D][rerun.datatypes.UVec2D]."""

    @staticmethod
    def native_to_pa_array_override(data: UVec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        points = flat_np_uint32_array_from_array_like(data, 2)
        return pa.FixedSizeListArray.from_arrays(points, type=data_type)
