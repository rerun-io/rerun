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

    @classmethod
    def auto(cls, val: str | int | QueryKind) -> QueryKind:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, QueryKind):
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


QueryKindLike = Union[QueryKind, Literal["LatestAt", "TimeRange", "latestat", "timerange"], int]
QueryKindArrayLike = Union[QueryKindLike, Sequence[QueryKindLike]]


class QueryKindType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.QueryKind"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class QueryKindBatch(BaseBatch[QueryKindArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = QueryKindType()

    @staticmethod
    def _native_to_pa_array(data: QueryKindArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (QueryKind, int, str)):
            data = [data]

        pa_data = [QueryKind.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
