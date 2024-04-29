from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import AABB2DArrayLike


class AABB2DExt:
    """Extension for [AABB2D][rerun.datatypes.AABB2D]."""

    @staticmethod
    def native_to_pa_array_override(data: AABB2DArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import AABB2D, Vec2DBatch

        if isinstance(data, AABB2D):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                Vec2DBatch._native_to_pa_array([aabb.min for aabb in data], data_type["min"].type),
                Vec2DBatch._native_to_pa_array([aabb.max for aabb in data], data_type["min"].type),
            ],
            fields=list(data_type),
        )
