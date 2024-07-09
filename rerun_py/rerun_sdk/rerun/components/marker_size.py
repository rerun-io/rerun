# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/marker_size.fbs".

# You can extend this class by creating a "MarkerSizeExt" class in "marker_size_ext.py".

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

__all__ = ["MarkerSize", "MarkerSizeArrayLike", "MarkerSizeBatch", "MarkerSizeLike", "MarkerSizeType"]


@define(init=False)
class MarkerSize(ComponentMixin):
    """**Component**: Radius of a marker of a point in e.g. a 2D plot, measured in UI points."""

    _BATCH_TYPE = None

    def __init__(self: Any, value: MarkerSizeLike):
        """Create a new instance of the MarkerSize component."""

        # You can define your own __init__ function as a member of MarkerSizeExt in marker_size_ext.py
        self.__attrs_init__(value=value)

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of MarkerSizeExt in marker_size_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


if TYPE_CHECKING:
    MarkerSizeLike = Union[MarkerSize, float]
else:
    MarkerSizeLike = Any

MarkerSizeArrayLike = Union[MarkerSize, Sequence[MarkerSizeLike], float, npt.ArrayLike]


class MarkerSizeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.MarkerSize"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), self._TYPE_NAME)


class MarkerSizeBatch(BaseBatch[MarkerSizeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = MarkerSizeType()

    @staticmethod
    def _native_to_pa_array(data: MarkerSizeArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)


# This is patched in late to avoid circular dependencies.
MarkerSize._BATCH_TYPE = MarkerSizeBatch  # type: ignore[assignment]
