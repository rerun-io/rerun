from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from .._validators import flat_np_int32_array_from_array_like

if TYPE_CHECKING:
    from . import IVec3DArrayLike


class IVec3DExt:
    """Extension for [IVec3D][rerun.datatypes.IVec3D]."""

    @staticmethod
    def native_to_pa_array_override(data: IVec3DArrayLike, data_type: pa.DataType) -> pa.Array:
        points = flat_np_int32_array_from_array_like(data, 3)
        return pa.FixedSizeListArray.from_arrays(points, type=data_type)
