# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/color.fbs".

# You can extend this class by creating a "ColorExt" class in "color_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .color_ext import ColorExt

__all__ = ["Color", "ColorArrayLike", "ColorBatch", "ColorLike", "ColorType"]


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

    def __init__(self: Any, rgba: ColorLike):
        """Create a new instance of the Color datatype."""

        # You can define your own __init__ function as a member of ColorExt in color_ext.py
        self.__attrs_init__(rgba=rgba)

    rgba: int = field(
        converter=ColorExt.rgba__field_converter_override,  # type: ignore[misc]
    )

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of ColorExt in color_ext.py
        return np.asarray(self.rgba, dtype=dtype)

    def __int__(self) -> int:
        return int(self.rgba)


if TYPE_CHECKING:
    ColorLike = Union[Color, int, Sequence[Union[int, float]], npt.NDArray[Union[np.uint8, np.float32, np.float64]]]
else:
    ColorLike = Any

ColorArrayLike = Union[
    Color,
    Sequence[ColorLike],
    int,
    Sequence[Union[int, float]],
    Sequence[Sequence[Union[int, float]]],
    npt.NDArray[Union[np.uint8, np.uint32, np.float32, np.float64]],
]


class ColorType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Color"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint32(), self._TYPE_NAME)


class ColorBatch(BaseBatch[ColorArrayLike]):
    _ARROW_TYPE = ColorType()

    @staticmethod
    def _native_to_pa_array(data: ColorArrayLike, data_type: pa.DataType) -> pa.Array:
        return ColorExt.native_to_pa_array_override(data, data_type)


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ColorType())


if hasattr(ColorExt, "deferred_patch_class"):
    ColorExt.deferred_patch_class(Color)
