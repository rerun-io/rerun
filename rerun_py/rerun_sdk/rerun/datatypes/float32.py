# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/float32.fbs".

# You can extend this class by creating a "Float32Ext" class in "float32_ext.py".

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

__all__ = ["Float32", "Float32ArrayLike", "Float32Batch", "Float32Like"]


@define(init=False)
class Float32:
    """**Datatype**: A single-precision 32-bit IEEE 754 floating point number."""

    def __init__(self: Any, value: Float32Like) -> None:
        """Create a new instance of the Float32 datatype."""

        # You can define your own __init__ function as a member of Float32Ext in float32_ext.py
        self.__attrs_init__(value=value)

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Float32Ext in float32_ext.py
        return np.asarray(self.value, dtype=dtype, copy=copy)

    def __float__(self) -> float:
        return float(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


if TYPE_CHECKING:
    Float32Like = Union[Float32, float]
else:
    Float32Like = Any

Float32ArrayLike = Union[
    Float32, Sequence[Float32Like], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


class Float32Batch(BaseBatch[Float32ArrayLike]):
    _ARROW_DATATYPE = pa.float32()

    @staticmethod
    def _native_to_pa_array(data: Float32ArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)
