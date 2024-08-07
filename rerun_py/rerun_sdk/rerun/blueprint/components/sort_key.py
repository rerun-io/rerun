# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/sort_key.fbs".

# You can extend this class by creating a "SortKeyExt" class in "sort_key_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["SortKey", "SortKeyArrayLike", "SortKeyBatch", "SortKeyLike", "SortKeyType"]


from enum import Enum


class SortKey(Enum):
    """**Component**: Primary element by which to group by in a temporal data table."""

    Entity = 1
    """Group by entity."""

    Time = 2
    """Group by instance."""

    @classmethod
    def auto(cls, val: str | int | SortKey) -> SortKey:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, SortKey):
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


SortKeyLike = Union[SortKey, Literal["Entity", "Time", "entity", "time"], int]
SortKeyArrayLike = Union[SortKeyLike, Sequence[SortKeyLike]]


class SortKeyType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.SortKey"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class SortKeyBatch(BaseBatch[SortKeyArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = SortKeyType()

    @staticmethod
    def _native_to_pa_array(data: SortKeyArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (SortKey, int, str)):
            data = [data]

        pa_data = [SortKey.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
