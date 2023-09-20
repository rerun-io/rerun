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

        field_vertex_indices = data_type.field("vertex_indices")
        vertex_indices = pa.array(
            [np.array(datum.vertex_indices).flatten() for datum in data],
            type=field_vertex_indices.type,
        )

        return pa.StructArray.from_arrays(
            arrays=[vertex_indices],
            fields=[field_vertex_indices],
        )
