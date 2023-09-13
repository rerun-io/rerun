# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/color.fbs".

# You can extend this class by creating a "ColorExt" class in "color_ext.py".

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
from .color_ext import ColorExt

__all__ = ["Color", "ColorArray", "ColorArrayLike", "ColorLike", "ColorType"]


@define
class Color(ColorExt):
    """
    An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.

    The color is stored as a 32-bit integer, where the most significant
    byte is `R` and the least significant byte is `A`.

    Float colors are assumed to be in 0-1 gamma sRGB space.
    All other colors are assumed to be in 0-255 gamma sRGB space.
    If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
    """

    # You can define your own __init__ function as a member of ColorExt in color_ext.py

    rgba: int = field(
        converter=ColorExt.rgba__field_converter_override,  # type: ignore[misc]
    )

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of ColorExt in color_ext.py
        return np.asarray(self.rgba, dtype=dtype)

    def __int__(self) -> int:
        return int(self.rgba)


if TYPE_CHECKING:
    ColorLike = Union[Color, int, Sequence[int], npt.NDArray[Union[np.uint8, np.float32, np.float64]]]
else:
    ColorLike = Any

ColorArrayLike = Union[
    Color,
    Sequence[ColorLike],
    int,
    Sequence[Sequence[int]],
    npt.NDArray[Union[np.uint8, np.uint32, np.float32, np.float64]],
]


# --- Arrow support ---


class ColorType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint32(), "rerun.datatypes.Color")


class ColorArray(BaseExtensionArray[ColorArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Color"
    _EXTENSION_TYPE = ColorType

    @staticmethod
    def _native_to_pa_array(data: ColorArrayLike, data_type: pa.DataType) -> pa.Array:
        return ColorExt.native_to_pa_array_override(data, data_type)


ColorType._ARRAY_TYPE = ColorArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ColorType())
