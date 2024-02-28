# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/marker_shape.fbs".

# You can extend this class by creating a "MarkerShapeExt" class in "marker_shape_ext.py".

from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["MarkerShape", "MarkerShapeArrayLike", "MarkerShapeBatch", "MarkerShapeLike", "MarkerShapeType"]


from enum import Enum


class MarkerShape(Enum):
    """**Component**: Shape of a marker."""

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


MarkerShapeLike = Union[MarkerShape, str]
MarkerShapeArrayLike = Union[MarkerShapeLike, Sequence[MarkerShapeLike]]


class MarkerShapeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.MarkerShape"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union(
                [
                    pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                    pa.field("Circle", pa.null(), nullable=True, metadata={}),
                    pa.field("Diamond", pa.null(), nullable=True, metadata={}),
                    pa.field("Square", pa.null(), nullable=True, metadata={}),
                    pa.field("Cross", pa.null(), nullable=True, metadata={}),
                    pa.field("Plus", pa.null(), nullable=True, metadata={}),
                    pa.field("Up", pa.null(), nullable=True, metadata={}),
                    pa.field("Down", pa.null(), nullable=True, metadata={}),
                    pa.field("Left", pa.null(), nullable=True, metadata={}),
                    pa.field("Right", pa.null(), nullable=True, metadata={}),
                    pa.field("Asterisk", pa.null(), nullable=True, metadata={}),
                ]
            ),
            self._TYPE_NAME,
        )


class MarkerShapeBatch(BaseBatch[MarkerShapeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = MarkerShapeType()

    @staticmethod
    def _native_to_pa_array(data: MarkerShapeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (MarkerShape, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, MarkerShape):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(MarkerShape, value):
                    types.append(MarkerShape[value].value)  # fast path
                elif value.lower() == "circle":
                    types.append(MarkerShape.Circle.value)
                elif value.lower() == "diamond":
                    types.append(MarkerShape.Diamond.value)
                elif value.lower() == "square":
                    types.append(MarkerShape.Square.value)
                elif value.lower() == "cross":
                    types.append(MarkerShape.Cross.value)
                elif value.lower() == "plus":
                    types.append(MarkerShape.Plus.value)
                elif value.lower() == "up":
                    types.append(MarkerShape.Up.value)
                elif value.lower() == "down":
                    types.append(MarkerShape.Down.value)
                elif value.lower() == "left":
                    types.append(MarkerShape.Left.value)
                elif value.lower() == "right":
                    types.append(MarkerShape.Right.value)
                elif value.lower() == "asterisk":
                    types.append(MarkerShape.Asterisk.value)
                else:
                    raise ValueError(f"Unknown MarkerShape kind: {value}")
            else:
                raise ValueError(f"Unknown MarkerShape kind: {value}")

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
        ]
        children = (1 + 10) * [pa.nulls(len(data))]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
