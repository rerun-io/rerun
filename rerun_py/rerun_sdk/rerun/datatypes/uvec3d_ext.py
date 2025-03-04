from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .._validators import flat_np_uint32_array_from_array_like

if TYPE_CHECKING:
    from . import UVec3DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class UVec3DExt:
    """Extension for [UVec3D][rerun.datatypes.UVec3D]."""

    @staticmethod
    def native_to_pa_array_override(data: UVec3DArrayLike, data_type: pa.DataType) -> pa.Array:
        points = flat_np_uint32_array_from_array_like(data, 3)
        return pa.FixedSizeListArray.from_arrays(points, type=data_type)
