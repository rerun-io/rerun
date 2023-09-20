# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/uvec2d.fbs".

# You can extend this class by creating a "UVec2DExt" class in "uvec2d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    to_np_uint32,
)

__all__ = ["UVec2D", "UVec2DArray", "UVec2DArrayLike", "UVec2DLike", "UVec2DType"]


@define
class UVec2D:
    """A uint32 vector in 2D space."""

    # You can define your own __init__ function as a member of UVec2DExt in uvec2d_ext.py

    xy: npt.NDArray[np.uint32] = field(converter=to_np_uint32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UVec2DExt in uvec2d_ext.py
        return np.asarray(self.xy, dtype=dtype)


if TYPE_CHECKING:
    UVec2DLike = Union[UVec2D, npt.NDArray[Any], npt.ArrayLike, Sequence[int]]
else:
    UVec2DLike = Any

UVec2DArrayLike = Union[
    UVec2D, Sequence[UVec2DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[int]], Sequence[int]
]


# --- Arrow support ---


class UVec2DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={}), 2), "rerun.datatypes.UVec2D"
        )


class UVec2DArray(BaseExtensionArray[UVec2DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.UVec2D"
    _EXTENSION_TYPE = UVec2DType

    @staticmethod
    def _native_to_pa_array(data: UVec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in uvec2d_ext.py


UVec2DType._ARRAY_TYPE = UVec2DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(UVec2DType())
