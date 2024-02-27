# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/mesh_properties.fbs".

# You can extend this class by creating a "MeshPropertiesExt" class in "mesh_properties_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .._converters import (
    to_np_uint32,
)
from .mesh_properties_ext import MeshPropertiesExt

__all__ = [
    "MeshProperties",
    "MeshPropertiesArrayLike",
    "MeshPropertiesBatch",
    "MeshPropertiesLike",
    "MeshPropertiesType",
]


@define(init=False)
class MeshProperties(MeshPropertiesExt):
    """**Datatype**: Optional triangle indices for a mesh."""

    def __init__(self: Any, indices: npt.ArrayLike | None = None):
        """
        Create a new instance of the MeshProperties datatype.

        Parameters
        ----------
        indices:
            A flattened array of vertex indices that describe the mesh's triangles.

            Its length must be divisible by 3.

        """

        # You can define your own __init__ function as a member of MeshPropertiesExt in mesh_properties_ext.py
        self.__attrs_init__(indices=indices)

    indices: npt.NDArray[np.uint32] | None = field(default=None, converter=to_np_uint32)
    # A flattened array of vertex indices that describe the mesh's triangles.
    #
    # Its length must be divisible by 3.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of MeshPropertiesExt in mesh_properties_ext.py
        return np.asarray(self.indices, dtype=dtype)


MeshPropertiesLike = MeshProperties
MeshPropertiesArrayLike = Union[
    MeshProperties,
    Sequence[MeshPropertiesLike],
]


class MeshPropertiesType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.MeshProperties"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field(
                        "indices",
                        pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={})),
                        nullable=True,
                        metadata={},
                    )
                ]
            ),
            self._TYPE_NAME,
        )


class MeshPropertiesBatch(BaseBatch[MeshPropertiesArrayLike]):
    _ARROW_TYPE = MeshPropertiesType()

    @staticmethod
    def _native_to_pa_array(data: MeshPropertiesArrayLike, data_type: pa.DataType) -> pa.Array:
        return MeshPropertiesExt.native_to_pa_array_override(data, data_type)
