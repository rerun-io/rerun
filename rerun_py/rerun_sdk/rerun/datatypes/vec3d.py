# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/vec3d.fbs".

# You can extend this class by creating a "Vec3DExt" class in "vec3d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .._converters import (
    to_np_float32,
)
from .vec3d_ext import Vec3DExt

__all__ = ["Vec3D", "Vec3DArrayLike", "Vec3DBatch", "Vec3DLike", "Vec3DType"]


@define
class Vec3D(Vec3DExt):
    """A vector in 3D space."""

    # You can define your own __init__ function as a member of Vec3DExt in vec3d_ext.py

    xyz: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Vec3DExt in vec3d_ext.py
        return np.asarray(self.xyz, dtype=dtype)


if TYPE_CHECKING:
    Vec3DLike = Union[Vec3D, npt.NDArray[Any], npt.ArrayLike, Sequence[float]]
else:
    Vec3DLike = Any

Vec3DArrayLike = Union[
    Vec3D, Sequence[Vec3DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


class Vec3DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Vec3D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3), self._TYPE_NAME
        )


class Vec3DBatch(BaseBatch[Vec3DArrayLike]):
    _ARROW_TYPE = Vec3DType()

    @staticmethod
    def _native_to_pa_array(data: Vec3DArrayLike, data_type: pa.DataType) -> pa.Array:
        return Vec3DExt.native_to_pa_array_override(data, data_type)


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Vec3DType())


if hasattr(Vec3DExt, "deferred_patch_class"):
    Vec3DExt.deferred_patch_class(Vec3D)
