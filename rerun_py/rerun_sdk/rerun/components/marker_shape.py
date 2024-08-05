# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/marker_shape.fbs".

# You can extend this class by creating a "MarkerShapeExt" class in "marker_shape_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["MarkerShape", "MarkerShapeArrayLike", "MarkerShapeBatch", "MarkerShapeLike", "MarkerShapeType"]


from enum import Enum


class MarkerShape(Enum):
    """**Component**: The visual appearance of a point in e.g. a 2D plot."""

    Circle = 0
    """`⏺`"""

    Diamond = 1
    """`◆`"""

    Square = 2
    """`◼️`"""

    Cross = 3
    """`x`"""

    Plus = 4
    """`+`"""

    Up = 5
    """`▲`"""

    Down = 6
    """`▼`"""

    Left = 7
    """`◀`"""

    Right = 8
    """`▶`"""

    Asterisk = 9
    """`*`"""

    @classmethod
    def auto(cls, val: str | int | MarkerShape) -> MarkerShape:
        """Best-effort converter."""
        if isinstance(val, MarkerShape):
            return val
        if isinstance(val, int):
            return cls(val)
        try:
            return cls[val]
        except KeyError:
            val_lower = val.lower()
            for variant in cls:
                if variant.name.lower() == val_lower:
                    return variant
        raise ValueError(f"Cannot convert {val} to {cls.__name__}")

    def __str__(self) -> str:
        """Returns the variant name."""
        return self.name


MarkerShapeLike = Union[
    MarkerShape,
    Literal[
        "Asterisk",
        "Circle",
        "Cross",
        "Diamond",
        "Down",
        "Left",
        "Plus",
        "Right",
        "Square",
        "Up",
        "asterisk",
        "circle",
        "cross",
        "diamond",
        "down",
        "left",
        "plus",
        "right",
        "square",
        "up",
    ],
    int,
]
MarkerShapeArrayLike = Union[MarkerShapeLike, Sequence[MarkerShapeLike]]


class MarkerShapeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.MarkerShape"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class MarkerShapeBatch(BaseBatch[MarkerShapeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = MarkerShapeType()

    @staticmethod
    def _native_to_pa_array(data: MarkerShapeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (MarkerShape, int, str)):
            data = [data]

        pa_data = [MarkerShape.auto(v).value for v in data]

        return pa.array(pa_data, type=data_type)
