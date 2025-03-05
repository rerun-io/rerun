# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/rgba32.fbs".

# You can extend this class by creating a "Rgba32Ext" class in "rgba32_ext.py".

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
from .rgba32_ext import Rgba32Ext

__all__ = ["Rgba32", "Rgba32ArrayLike", "Rgba32Batch", "Rgba32Like"]


@define(init=False)
class Rgba32(Rgba32Ext):
    """
    **Datatype**: An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.

    The color is stored as a 32-bit integer, where the most significant
    byte is `R` and the least significant byte is `A`.

    Float colors are assumed to be in 0-1 gamma sRGB space.
    All other colors are assumed to be in 0-255 gamma sRGB space.
    If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
    """

    def __init__(self: Any, rgba: Rgba32Like) -> None:
        """Create a new instance of the Rgba32 datatype."""

        # You can define your own __init__ function as a member of Rgba32Ext in rgba32_ext.py
        self.__attrs_init__(rgba=rgba)

    rgba: int = field(
        converter=Rgba32Ext.rgba__field_converter_override,  # type: ignore[misc]
    )

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Rgba32Ext in rgba32_ext.py
        return np.asarray(self.rgba, dtype=dtype, copy=copy)

    def __int__(self) -> int:
        return int(self.rgba)

    def __hash__(self) -> int:
        return hash(self.rgba)


if TYPE_CHECKING:
    Rgba32Like = Union[
        Rgba32,
        int,
        Sequence[Union[int, float]],
        npt.NDArray[Union[np.uint8, np.float32, np.float64]],
    ]
else:
    Rgba32Like = Any

Rgba32ArrayLike = Union[
    Rgba32,
    Sequence[Rgba32Like],
    int,
    npt.ArrayLike,
]


class Rgba32Batch(BaseBatch[Rgba32ArrayLike]):
    _ARROW_DATATYPE = pa.uint32()

    @staticmethod
    def _native_to_pa_array(data: Rgba32ArrayLike, data_type: pa.DataType) -> pa.Array:
        return Rgba32Ext.native_to_pa_array_override(data, data_type)
