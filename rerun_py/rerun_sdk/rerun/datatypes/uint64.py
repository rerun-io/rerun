# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/uint64.fbs".

# You can extend this class by creating a "UInt64Ext" class in "uint64_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType

__all__ = ["UInt64", "UInt64ArrayLike", "UInt64Batch", "UInt64Like", "UInt64Type"]


@define(init=False)
class UInt64:
    """**Datatype**: A 64bit unsigned integer."""

    def __init__(self: Any, value: UInt64Like):
        """Create a new instance of the UInt64 datatype."""

        # You can define your own __init__ function as a member of UInt64Ext in uint64_ext.py
        self.__attrs_init__(value=value)

    value: int = field(converter=int)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UInt64Ext in uint64_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __int__(self) -> int:
        return int(self.value)


UInt64Like = UInt64
UInt64ArrayLike = Union[
    UInt64,
    Sequence[UInt64Like],
]


class UInt64Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.UInt64"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint64(), self._TYPE_NAME)


class UInt64Batch(BaseBatch[UInt64ArrayLike]):
    _ARROW_TYPE = UInt64Type()

    @staticmethod
    def _native_to_pa_array(data: UInt64ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in uint64_ext.py
