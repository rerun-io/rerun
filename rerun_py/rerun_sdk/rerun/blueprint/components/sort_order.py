# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/sort_order.fbs".

# You can extend this class by creating a "SortOrderExt" class in "sort_order_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["SortOrder", "SortOrderArrayLike", "SortOrderBatch", "SortOrderLike", "SortOrderType"]


from enum import Enum


class SortOrder(Enum):
    """**Component**: Sort order for data table."""

    Ascending = 1
    """Ascending"""

    Descending = 2
    """Descending"""

    def __str__(self) -> str:
        """Returns the variant name."""
        if self == SortOrder.Ascending:
            return "Ascending"
        elif self == SortOrder.Descending:
            return "Descending"
        else:
            raise ValueError("Unknown enum variant")


SortOrderLike = Union[SortOrder, Literal["Ascending", "Descending", "ascending", "descending"]]
SortOrderArrayLike = Union[SortOrderLike, Sequence[SortOrderLike]]


class SortOrderType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.SortOrder"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("Ascending", pa.null(), nullable=True, metadata={}),
                pa.field("Descending", pa.null(), nullable=True, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class SortOrderBatch(BaseBatch[SortOrderArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = SortOrderType()

    @staticmethod
    def _native_to_pa_array(data: SortOrderArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (SortOrder, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, SortOrder):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(SortOrder, value):
                    types.append(SortOrder[value].value)  # fast path
                elif value.lower() == "ascending":
                    types.append(SortOrder.Ascending.value)
                elif value.lower() == "descending":
                    types.append(SortOrder.Descending.value)
                else:
                    raise ValueError(f"Unknown SortOrder kind: {value}")
            else:
                raise ValueError(f"Unknown SortOrder kind: {value}")

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
