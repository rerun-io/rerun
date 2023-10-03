# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/line_strip2d.fbs".

# You can extend this class by creating a "LineStrip2DExt" class in "line_strip2d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .line_strip2d_ext import LineStrip2DExt

__all__ = ["LineStrip2D", "LineStrip2DArrayLike", "LineStrip2DBatch", "LineStrip2DLike", "LineStrip2DType"]


@define(init=False)
class LineStrip2D(LineStrip2DExt):
    r"""
    A line strip in 2D space.

    A line strip is a list of points connected by line segments. It can be used to draw
    approximations of smooth curves.

    The points will be connected in order, like so:
    ```text
           2------3     5
          /        \   /
    0----1          \ /
                     4
    ```
    """

    def __init__(self: Any, points: LineStrip2DLike):
        """Create a new instance of the LineStrip2D component."""

        # You can define your own __init__ function as a member of LineStrip2DExt in line_strip2d_ext.py
        self.__attrs_init__(points=points)

    points: list[datatypes.Vec2D] = field()


if TYPE_CHECKING:
    LineStrip2DLike = Union[LineStrip2D, datatypes.Vec2DArrayLike, npt.NDArray[np.float32]]
else:
    LineStrip2DLike = Any

LineStrip2DArrayLike = Union[LineStrip2D, Sequence[LineStrip2DLike], npt.NDArray[np.float32]]


class LineStrip2DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.LineStrip2D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 2),
                    nullable=False,
                    metadata={},
                )
            ),
            self._TYPE_NAME,
        )


class LineStrip2DBatch(BaseBatch[LineStrip2DArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = LineStrip2DType()

    @staticmethod
    def _native_to_pa_array(data: LineStrip2DArrayLike, data_type: pa.DataType) -> pa.Array:
        return LineStrip2DExt.native_to_pa_array_override(data, data_type)
