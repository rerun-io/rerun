# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/radius.fbs".

# You can extend this class by creating a "RadiusExt" class in "radius_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)
from .radius_ext import RadiusExt

__all__ = ["Radius", "RadiusArrayLike", "RadiusBatch", "RadiusLike", "RadiusType"]


@define(init=False)
class Radius(RadiusExt, ComponentMixin):
    """
    **Component**: The radius of something, e.g. a point.

    Internally, positive values indicate scene units, whereas negative values
    are interpreted as ui points.

    Ui points are independent of zooming in Views, but are sensitive to the application ui scaling.
    At 100% ui scaling, ui points are equal to pixels
    The Viewer's ui scaling defaults to the OS scaling which typically is 100% for full HD screens and 200% for 4k screens.
    """

    _BATCH_TYPE = None

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

    def __hash__(self) -> int:
        return hash(self.value)


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


# This is patched in late to avoid circular dependencies.
Radius._BATCH_TYPE = RadiusBatch  # type: ignore[assignment]
