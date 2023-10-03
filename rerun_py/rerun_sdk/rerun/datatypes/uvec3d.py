# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/uvec3d.fbs".

# You can extend this class by creating a "UVec3DExt" class in "uvec3d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .._converters import (
    to_np_uint32,
)

__all__ = ["UVec3D", "UVec3DArrayLike", "UVec3DBatch", "UVec3DLike", "UVec3DType"]


@define(init=False)
class UVec3D:
    """A uint32 vector in 3D space."""

    def __init__(self: Any, xyz: UVec3DLike):
        """Create a new instance of the UVec3D datatype."""

        # You can define your own __init__ function as a member of UVec3DExt in uvec3d_ext.py
        self.__attrs_init__(xyz=xyz)

    xyz: npt.NDArray[np.uint32] = field(converter=to_np_uint32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UVec3DExt in uvec3d_ext.py
        return np.asarray(self.xyz, dtype=dtype)


if TYPE_CHECKING:
    UVec3DLike = Union[UVec3D, npt.NDArray[Any], npt.ArrayLike, Sequence[int]]
else:
    UVec3DLike = Any

UVec3DArrayLike = Union[
    UVec3D, Sequence[UVec3DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[int]], Sequence[int]
]


class UVec3DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.UVec3D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={}), 3), self._TYPE_NAME
        )


class UVec3DBatch(BaseBatch[UVec3DArrayLike]):
    _ARROW_TYPE = UVec3DType()

    @staticmethod
    def _native_to_pa_array(data: UVec3DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in uvec3d_ext.py
