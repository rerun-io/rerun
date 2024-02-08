from __future__ import annotations

from enum import Enum
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import MarkerShape, MarkerShapeArrayLike, MarkerShapeLike


class MarkerShapeExt:
    """Extension for [MarkerShape][rerun.components.MarkerShape]."""

    class Shape(Enum):
        Circle = 1
        Diamond = 2
        Square = 3
        Cross = 4
        Plus = 5
        Up = 6
        Down = 7
        Left = 8
        Right = 9
        Asterisk = 10

    Circle: MarkerShape = None  # type: ignore[assignment]
    Diamond: MarkerShape = None  # type: ignore[assignment]
    Square: MarkerShape = None  # type: ignore[assignment]
    Cross: MarkerShape = None  # type: ignore[assignment]
    Plus: MarkerShape = None  # type: ignore[assignment]
    Up: MarkerShape = None  # type: ignore[assignment]
    Down: MarkerShape = None  # type: ignore[assignment]
    Left: MarkerShape = None  # type: ignore[assignment]
    Right: MarkerShape = None  # type: ignore[assignment]
    Asterisk: MarkerShape = None  # type: ignore[assignment]

    @staticmethod
    def shape__field_converter_override(data: MarkerShapeLike) -> int:
        if isinstance(data, int):
            return MarkerShapeExt.Shape(data).value
        elif isinstance(data, str):
            return MarkerShapeExt.Shape[data.title()].value
        else:
            # Must be a MarkerShape
            return data.shape

    @staticmethod
    def native_to_pa_array_override(data: MarkerShapeArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import MarkerShape

        # If it's the singular version, wrap it in an array
        if isinstance(data, (MarkerShape, int, str)):
            data = [data]

        # Apply the field-converter to every element
        data = [MarkerShapeExt.shape__field_converter_override(d) for d in data]

        array = np.asarray(data, dtype=np.uint8).flatten()
        return pa.array(array, type=data_type)

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.Circle = cls(cls.Shape.Circle.value)
        cls.Diamond = cls(cls.Shape.Diamond.value)
        cls.Square = cls(cls.Shape.Square.value)
        cls.Cross = cls(cls.Shape.Cross.value)
        cls.Plus = cls(cls.Shape.Plus.value)
        cls.Up = cls(cls.Shape.Up.value)
        cls.Down = cls(cls.Shape.Down.value)
        cls.Left = cls(cls.Shape.Left.value)
        cls.Right = cls(cls.Shape.Right.value)
        cls.Asterisk = cls(cls.Shape.Asterisk.value)
