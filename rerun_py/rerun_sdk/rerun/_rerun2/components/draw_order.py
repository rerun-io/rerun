# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import draworder_native_to_pa_array  # noqa: F401

__all__ = ["DrawOrder", "DrawOrderArray", "DrawOrderArrayLike", "DrawOrderLike", "DrawOrderType"]


@define
class DrawOrder:
    """
    Draw order used for the display order of 2D elements.

    Higher values are drawn on top of lower values.
    An entity can have only a single draw order component.
    Within an entity draw order is governed by the order of the components.

    Draw order for entities with the same draw order is generally undefined.
    """

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


if TYPE_CHECKING:
    DrawOrderLike = Union[DrawOrder, float]
else:
    DrawOrderLike = Any

DrawOrderArrayLike = Union[DrawOrder, Sequence[DrawOrderLike], float, npt.NDArray[np.float32]]


# --- Arrow support ---


class DrawOrderType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.draw_order")


class DrawOrderArray(BaseExtensionArray[DrawOrderArrayLike]):
    _EXTENSION_NAME = "rerun.draw_order"
    _EXTENSION_TYPE = DrawOrderType

    @staticmethod
    def _native_to_pa_array(data: DrawOrderArrayLike, data_type: pa.DataType) -> pa.Array:
        return draworder_native_to_pa_array(data, data_type)


DrawOrderType._ARRAY_TYPE = DrawOrderArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(DrawOrderType())
