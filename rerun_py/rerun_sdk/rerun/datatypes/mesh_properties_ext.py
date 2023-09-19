from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import MeshPropertiesArrayLike


class MeshPropertiesExt:
    @staticmethod
    def native_to_pa_array_override(data: MeshPropertiesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import MeshProperties

        if isinstance(data, MeshProperties):
            data = [data]

        field_triangle_indices = data_type.field("triangle_indices")
        triangle_indices = pa.array(
            [np.array(datum.triangle_indices).flatten() for datum in data],
            type=field_triangle_indices.type,
        )

        return pa.StructArray.from_arrays(
            arrays=[triangle_indices],
            fields=[field_triangle_indices],
        )
