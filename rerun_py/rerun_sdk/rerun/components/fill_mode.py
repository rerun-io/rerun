# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/fill_mode.fbs".

# You can extend this class by creating a "FillModeExt" class in "fill_mode_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["FillMode", "FillModeArrayLike", "FillModeBatch", "FillModeLike", "FillModeType"]


from enum import Enum


class FillMode(Enum):
    """**Component**: How a geometric shape is drawn and colored."""

    Wireframe = 1
    """
    Lines are drawn around the edges of the shape.

    The interior (2D) or surface (3D) are not drawn.
    """

    Solid = 2
    """
    The interior (2D) or surface (3D) is filled with a single color.

    Lines are not drawn.
    """


FillModeLike = Union[FillMode, Literal["wireframe", "solid"]]
FillModeArrayLike = Union[FillModeLike, Sequence[FillModeLike]]


class FillModeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.FillMode"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("Wireframe", pa.null(), nullable=True, metadata={}),
                pa.field("Solid", pa.null(), nullable=True, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class FillModeBatch(BaseBatch[FillModeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = FillModeType()

    @staticmethod
    def _native_to_pa_array(data: FillModeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (FillMode, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, FillMode):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(FillMode, value):
                    types.append(FillMode[value].value)  # fast path
                elif value.lower() == "wireframe":
                    types.append(FillMode.Wireframe.value)
                elif value.lower() == "solid":
                    types.append(FillMode.Solid.value)
                else:
                    raise ValueError(f"Unknown FillMode kind: {value}")
            else:
                raise ValueError(f"Unknown FillMode kind: {value}")

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
        ]
        children = (1 + 2) * [pa.nulls(len(data))]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
