# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/time_int.fbs".

# You can extend this class by creating a "TimeIntExt" class in "time_int_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType

__all__ = ["TimeInt", "TimeIntArrayLike", "TimeIntBatch", "TimeIntLike", "TimeIntType"]


@define(init=False)
class TimeInt:
    """**Datatype**: A 64-bit number describing either nanoseconds OR sequence numbers."""

    def __init__(self: Any, value: TimeIntLike):
        """Create a new instance of the TimeInt datatype."""

        # You can define your own __init__ function as a member of TimeIntExt in time_int_ext.py
        self.__attrs_init__(value=value)

    value: int = field(converter=int)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of TimeIntExt in time_int_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __int__(self) -> int:
        return int(self.value)


if TYPE_CHECKING:
    TimeIntLike = Union[TimeInt, int]
else:
    TimeIntLike = Any

TimeIntArrayLike = Union[
    TimeInt,
    Sequence[TimeIntLike],
]


class TimeIntType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.TimeInt"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.int64(), self._TYPE_NAME)


class TimeIntBatch(BaseBatch[TimeIntArrayLike]):
    _ARROW_TYPE = TimeIntType()

    @staticmethod
    def _native_to_pa_array(data: TimeIntArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.int64).flatten()
        return pa.array(array, type=data_type)
