# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/uvec4d.fbs".

# You can extend this class by creating a "UVec4DExt" class in "uvec4d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)
from .._converters import (
    to_np_uint32,
)

__all__ = ["UVec4D", "UVec4DArrayLike", "UVec4DBatch", "UVec4DLike"]


@define(init=False)
class UVec4D:
    """**Datatype**: A uint vector in 4D space."""

    def __init__(self: Any, xyzw: UVec4DLike):
        """Create a new instance of the UVec4D datatype."""

        # You can define your own __init__ function as a member of UVec4DExt in uvec4d_ext.py
        self.__attrs_init__(xyzw=xyzw)

    xyzw: npt.NDArray[np.uint32] = field(converter=to_np_uint32)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UVec4DExt in uvec4d_ext.py
        return np.asarray(self.xyzw, dtype=dtype, copy=copy)


if TYPE_CHECKING:
    UVec4DLike = Union[UVec4D, npt.NDArray[Any], npt.ArrayLike, Sequence[int]]
else:
    UVec4DLike = Any

UVec4DArrayLike = Union[
    UVec4D, Sequence[UVec4DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[int]], Sequence[int]
]


class UVec4DBatch(BaseBatch[UVec4DArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={}), 4)

    @staticmethod
    def _native_to_pa_array(data: UVec4DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of UVec4D not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in uvec4d_ext.py
