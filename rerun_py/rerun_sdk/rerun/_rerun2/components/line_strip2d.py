# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/line_strip2d.fbs".


from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import line_strip2d__native_to_pa_array_override  # noqa: F401

__all__ = ["LineStrip2D", "LineStrip2DArray", "LineStrip2DArrayLike", "LineStrip2DLike", "LineStrip2DType"]


@define
class LineStrip2D:
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

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    points: list[datatypes.Vec2D] = field()


if TYPE_CHECKING:
    LineStrip2DLike = Union[LineStrip2D, datatypes.Vec2DArrayLike, npt.NDArray[np.float32]]
else:
    LineStrip2DLike = Any

LineStrip2DArrayLike = Union[LineStrip2D, Sequence[LineStrip2DLike], npt.NDArray[np.float32]]


# --- Arrow support ---


class LineStrip2DType(BaseExtensionType):
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
            "rerun.linestrip2d",
        )


class LineStrip2DArray(BaseExtensionArray[LineStrip2DArrayLike]):
    _EXTENSION_NAME = "rerun.linestrip2d"
    _EXTENSION_TYPE = LineStrip2DType

    @staticmethod
    def _native_to_pa_array(data: LineStrip2DArrayLike, data_type: pa.DataType) -> pa.Array:
        return line_strip2d__native_to_pa_array_override(data, data_type)


LineStrip2DType._ARRAY_TYPE = LineStrip2DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(LineStrip2DType())
