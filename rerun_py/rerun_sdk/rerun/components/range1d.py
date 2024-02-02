# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/range1d.fbs".

# You can extend this class by creating a "Range1DExt" class in "range1d_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .._converters import (
    to_np_float32,
)

__all__ = ["Range1D", "Range1DArrayLike", "Range1DBatch", "Range1DLike", "Range1DType"]


@define(init=False)
class Range1D:
    """**Component**: A 1D range, specifying a lower and upper bound."""

    def __init__(self: Any, range: Range1DLike):
        """Create a new instance of the Range1D component."""

        # You can define your own __init__ function as a member of Range1DExt in range1d_ext.py
        self.__attrs_init__(range=range)

    range: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Range1DExt in range1d_ext.py
        return np.asarray(self.range, dtype=dtype)


Range1DLike = Range1D
Range1DArrayLike = Union[
    Range1D,
    Sequence[Range1DLike],
]


class Range1DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.Range1D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 2), self._TYPE_NAME
        )


class Range1DBatch(BaseBatch[Range1DArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = Range1DType()

    @staticmethod
    def _native_to_pa_array(data: Range1DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in range1d_ext.py
