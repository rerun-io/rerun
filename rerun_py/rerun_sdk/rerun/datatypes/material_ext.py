from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import MaterialArrayLike


class MaterialExt:
    @staticmethod
    def native_to_pa_array_override(data: MaterialArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Color, ColorArray, Material

        if isinstance(data, Material):
            data = [data]

        field_albedo_factors = data_type.field("albedo_factor")

        albedo_factors_no_null = [
            datum.albedo_factor if datum.albedo_factor is not None else Color(0x00000000) for datum in data
        ]
        albedo_factors = ColorArray.from_similar(albedo_factors_no_null).storage

        albedo_factors_null_mask = pa.array([datum.albedo_factor is None for datum in data])
        if len(albedo_factors_null_mask) > 0:
            albedo_factors.filter(albedo_factors_null_mask, null_selection_behavior="emit_null")

        return pa.StructArray.from_arrays(
            arrays=[albedo_factors],
            fields=[field_albedo_factors],
        )
