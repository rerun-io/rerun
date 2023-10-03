# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/scalar.fbs".

# You can extend this class by creating a "ScalarExt" class in "scalar_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .scalar_ext import ScalarExt

__all__ = ["Scalar", "ScalarArrayLike", "ScalarBatch", "ScalarLike", "ScalarType"]


@define(init=False)
class Scalar(ScalarExt):
    """
    A double-precision scalar.

    Used for time series plots.
    """

    def __init__(self: Any, value: ScalarLike):
        """Create a new instance of the Scalar component."""

        # You can define your own __init__ function as a member of ScalarExt in scalar_ext.py
        self.__attrs_init__(value=value)

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of ScalarExt in scalar_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


if TYPE_CHECKING:
    ScalarLike = Union[Scalar, float]
else:
    ScalarLike = Any

ScalarArrayLike = Union[Scalar, Sequence[ScalarLike], float, npt.NDArray[np.float64]]


class ScalarType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.Scalar"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float64(), self._TYPE_NAME)


class ScalarBatch(BaseBatch[ScalarArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ScalarType()

    @staticmethod
    def _native_to_pa_array(data: ScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        return ScalarExt.native_to_pa_array_override(data, data_type)
