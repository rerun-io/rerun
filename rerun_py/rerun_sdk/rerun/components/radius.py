# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/radius.fbs".

# You can extend this class by creating a "RadiusExt" class in "radius_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["Radius", "RadiusArrayLike", "RadiusBatch", "RadiusLike", "RadiusType"]


@define(init=False)
class Radius:
    """**Component**: A Radius component."""

    def __init__(self: Any, value: RadiusLike):
        """Create a new instance of the Radius component."""

        # You can define your own __init__ function as a member of RadiusExt in radius_ext.py
        self.__attrs_init__(value=value)

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of RadiusExt in radius_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


if TYPE_CHECKING:
    RadiusLike = Union[Radius, float]
else:
    RadiusLike = Any

RadiusArrayLike = Union[Radius, Sequence[RadiusLike], float, npt.ArrayLike]


class RadiusType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.Radius"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), self._TYPE_NAME)


class RadiusBatch(BaseBatch[RadiusArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = RadiusType()

    @staticmethod
    def _native_to_pa_array(data: RadiusArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)
