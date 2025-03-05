# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/uvec2d.fbs".

# You can extend this class by creating a "UVec2DExt" class in "uvec2d_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Union

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
from .uvec2d_ext import UVec2DExt

__all__ = ["UVec2D", "UVec2DArrayLike", "UVec2DBatch", "UVec2DLike"]


@define(init=False)
class UVec2D(UVec2DExt):
    """**Datatype**: A uint32 vector in 2D space."""

    def __init__(self: Any, xy: UVec2DLike) -> None:
        """Create a new instance of the UVec2D datatype."""

        # You can define your own __init__ function as a member of UVec2DExt in uvec2d_ext.py
        self.__attrs_init__(xy=xy)

    xy: npt.NDArray[np.uint32] = field(converter=to_np_uint32)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UVec2DExt in uvec2d_ext.py
        return np.asarray(self.xy, dtype=dtype, copy=copy)


if TYPE_CHECKING:
    UVec2DLike = Union[UVec2D, npt.NDArray[Any], npt.ArrayLike, Sequence[int]]
else:
    UVec2DLike = Any

UVec2DArrayLike = Union[
    UVec2D, Sequence[UVec2DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[int]], Sequence[int]
]


class UVec2DBatch(BaseBatch[UVec2DArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={}), 2)

    @staticmethod
    def _native_to_pa_array(data: UVec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        return UVec2DExt.native_to_pa_array_override(data, data_type)
