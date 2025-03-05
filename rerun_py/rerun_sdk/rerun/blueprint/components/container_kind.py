# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/container_kind.fbs".

# You can extend this class by creating a "ContainerKindExt" class in "container_kind_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
    ComponentDescriptor,
)

__all__ = ["ContainerKind", "ContainerKindArrayLike", "ContainerKindBatch", "ContainerKindLike"]


from enum import Enum


class ContainerKind(Enum):
    """**Component**: The kind of a blueprint container (tabs, grid, …)."""

    Tabs = 1
    """Put children in separate tabs"""

    Horizontal = 2
    """Order the children left to right"""

    Vertical = 3
    """Order the children top to bottom"""

    Grid = 4
    """Organize children in a grid layout"""

    @classmethod
    def auto(cls, val: str | int | ContainerKind) -> ContainerKind:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, ContainerKind):
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


ContainerKindLike = Union[
    ContainerKind, Literal["Grid", "Horizontal", "Tabs", "Vertical", "grid", "horizontal", "tabs", "vertical"], int
]
ContainerKindArrayLike = Union[ContainerKindLike, Sequence[ContainerKindLike]]


class ContainerKindBatch(BaseBatch[ContainerKindArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.uint8()
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.ContainerKind")

    @staticmethod
    def _native_to_pa_array(data: ContainerKindArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (ContainerKind, int, str)):
            data = [data]

        pa_data = [ContainerKind.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
