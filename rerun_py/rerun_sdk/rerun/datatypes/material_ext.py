from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import MaterialArrayLike


class MaterialExt:
    @staticmethod
    def native_to_pa_array_override(data: MaterialArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ColorArray, Material

        if isinstance(data, Material):
            data = [data]

        field_albedo_factors = data_type.field("albedo_factor")
        albedo_factors = ColorArray.from_similar([datum.albedo_factor for datum in data]).storage

        return pa.StructArray.from_arrays(
            arrays=[albedo_factors],
            fields=[field_albedo_factors],
        )
