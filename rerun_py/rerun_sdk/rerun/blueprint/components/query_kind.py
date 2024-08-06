# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/query_kind.fbs".

# You can extend this class by creating a "QueryKindExt" class in "query_kind_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["QueryKind", "QueryKindArrayLike", "QueryKindBatch", "QueryKindLike", "QueryKindType"]


from enum import Enum


class QueryKind(Enum):
    """**Component**: The kind of query displayed by the dataframe view."""

    LatestAt = 1
    """Query"""

    TimeRange = 2
    """Time range query."""

    def __str__(self) -> str:
        """Returns the variant name."""
        if self == QueryKind.LatestAt:
            return "LatestAt"
        elif self == QueryKind.TimeRange:
            return "TimeRange"
        else:
            raise ValueError("Unknown enum variant")


QueryKindLike = Union[QueryKind, Literal["LatestAt", "TimeRange", "latestat", "timerange"]]
QueryKindArrayLike = Union[QueryKindLike, Sequence[QueryKindLike]]


class QueryKindType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.QueryKind"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("LatestAt", pa.null(), nullable=True, metadata={}),
                pa.field("TimeRange", pa.null(), nullable=True, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class QueryKindBatch(BaseBatch[QueryKindArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = QueryKindType()

    @staticmethod
    def _native_to_pa_array(data: QueryKindArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (QueryKind, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, QueryKind):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(QueryKind, value):
                    types.append(QueryKind[value].value)  # fast path
                elif value.lower() == "latestat":
                    types.append(QueryKind.LatestAt.value)
                elif value.lower() == "timerange":
                    types.append(QueryKind.TimeRange.value)
                else:
                    raise ValueError(f"Unknown QueryKind kind: {value}")
            else:
                raise ValueError(f"Unknown QueryKind kind: {value}")

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
