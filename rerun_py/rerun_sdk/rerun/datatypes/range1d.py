# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/range1d.fbs".

# You can extend this class by creating a "Range1DExt" class in "range1d_ext.py".

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
    to_np_float64,
)
from .range1d_ext import Range1DExt

__all__ = ["Range1D", "Range1DArrayLike", "Range1DBatch", "Range1DLike"]


@define(init=False)
class Range1D(Range1DExt):
    """**Datatype**: A 1D range, specifying a lower and upper bound."""

    def __init__(self: Any, range: Range1DLike) -> None:
        """Create a new instance of the Range1D datatype."""

        # You can define your own __init__ function as a member of Range1DExt in range1d_ext.py
        self.__attrs_init__(range=range)

    range: npt.NDArray[np.float64] = field(converter=to_np_float64)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Range1DExt in range1d_ext.py
        return np.asarray(self.range, dtype=dtype, copy=copy)

    def __len__(self) -> int:
        # You can define your own __len__ function as a member of Range1DExt in range1d_ext.py
        return len(self.range)


if TYPE_CHECKING:
    Range1DLike = Union[Range1D, npt.NDArray[Any], npt.ArrayLike, Sequence[float], slice]
else:
    Range1DLike = Any

Range1DArrayLike = Union[
    Range1D, Sequence[Range1DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


class Range1DBatch(BaseBatch[Range1DArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.float64(), nullable=False, metadata={}), 2)

    @staticmethod
    def _native_to_pa_array(data: Range1DArrayLike, data_type: pa.DataType) -> pa.Array:
        return Range1DExt.native_to_pa_array_override(data, data_type)
