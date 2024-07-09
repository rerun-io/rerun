# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/material.fbs".

# You can extend this class by creating a "MaterialExt" class in "material_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from .material_ext import MaterialExt

__all__ = ["Material", "MaterialArrayLike", "MaterialBatch", "MaterialLike", "MaterialType"]


def _material__albedo_factor__special_field_converter_override(
    x: datatypes.Rgba32Like | None,
) -> datatypes.Rgba32 | None:
    if x is None:
        return None
    elif isinstance(x, datatypes.Rgba32):
        return x
    else:
        return datatypes.Rgba32(x)


@define(init=False)
class Material(MaterialExt):
    """**Datatype**: Material properties of a mesh, e.g. its color multiplier."""

    def __init__(self: Any, albedo_factor: datatypes.Rgba32Like | None = None):
        """
        Create a new instance of the Material datatype.

        Parameters
        ----------
        albedo_factor:
            Optional color multiplier.

        """

        # You can define your own __init__ function as a member of MaterialExt in material_ext.py
        self.__attrs_init__(albedo_factor=albedo_factor)

    albedo_factor: datatypes.Rgba32 | None = field(
        default=None, converter=_material__albedo_factor__special_field_converter_override
    )
    # Optional color multiplier.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


MaterialLike = Material
MaterialArrayLike = Union[
    Material,
    Sequence[MaterialLike],
]


class MaterialType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Material"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.struct([pa.field("albedo_factor", pa.uint32(), nullable=True, metadata={})]), self._TYPE_NAME
        )


class MaterialBatch(BaseBatch[MaterialArrayLike]):
    _ARROW_TYPE = MaterialType()

    @staticmethod
    def _native_to_pa_array(data: MaterialArrayLike, data_type: pa.DataType) -> pa.Array:
        return MaterialExt.native_to_pa_array_override(data, data_type)
