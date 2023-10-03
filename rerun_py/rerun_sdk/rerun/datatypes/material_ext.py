from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import MaterialArrayLike


class MaterialExt:
    """Extension for [Material][rerun.datatypes.Material]."""

    @staticmethod
    def native_to_pa_array_override(data: MaterialArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Material, Rgba32Type

        if isinstance(data, Material):
            data = [data]

        field_albedo_factors = data_type.field("albedo_factor")

        albedo_factors = pa.array(
            [datum.albedo_factor.rgba if datum.albedo_factor is not None else None for datum in data],
            type=Rgba32Type().storage_type,
        )

        return pa.StructArray.from_arrays(
            arrays=[albedo_factors],
            fields=[field_albedo_factors],
        )
