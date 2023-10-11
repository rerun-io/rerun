from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import MeshPropertiesArrayLike


class MeshPropertiesExt:
    """Extension for [MeshProperties][rerun.datatypes.MeshProperties]."""

    @staticmethod
    def native_to_pa_array_override(data: MeshPropertiesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import MeshProperties

        if isinstance(data, MeshProperties):
            data = [data]

        field_indices = data_type.field("indices")
        indices = pa.array(
            [np.array(datum.indices).flatten() for datum in data],
            type=field_indices.type,
        )

        return pa.StructArray.from_arrays(
            arrays=[indices],
            fields=[field_indices],
        )
