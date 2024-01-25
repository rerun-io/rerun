from __future__ import annotations

import collections
from typing import TYPE_CHECKING, cast

import pyarrow as pa

from rerun.datatypes.tensor_data_ext import TensorDataExt

if TYPE_CHECKING:
    from . import MaterialArrayLike


class MaterialExt:
    """Extension for [Material][rerun.datatypes.Material]."""

    @staticmethod
    def native_to_pa_array_override(data: MaterialArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Material, Rgba32Type, TensorDataType

        # If it's a sequence of a single Material, grab the first one
        if isinstance(data, collections.abc.Sequence):
            if len(data) > 0:
                if isinstance(data[0], Material):
                    if len(data) > 1:
                        raise ValueError("Materials do not support batches")
                    data = data[0]
        data = cast(Material, data)

        field_albedo_factors = data_type.field("albedo_factor")
        field_albedo_texture = data_type.field("albedo_texture")

        albedo_factors = pa.array(
            [data.albedo_factor.rgba if data.albedo_factor is not None else None],
            type=Rgba32Type().storage_type,
        )
        if data.albedo_texture is not None:
            albedo_texture = TensorDataExt.native_to_pa_array_override(
                data.albedo_texture, TensorDataType().storage_type
            )
        else:
            albedo_texture = pa.nulls(1, type=TensorDataType().storage_type)

        return pa.StructArray.from_arrays(
            arrays=[albedo_factors, albedo_texture],
            fields=[field_albedo_factors, field_albedo_texture],
        )
