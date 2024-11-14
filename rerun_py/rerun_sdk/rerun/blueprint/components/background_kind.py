# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/background_kind.fbs".

# You can extend this class by creating a "BackgroundKindExt" class in "background_kind_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
)

__all__ = ["BackgroundKind", "BackgroundKindArrayLike", "BackgroundKindBatch", "BackgroundKindLike"]


from enum import Enum


class BackgroundKind(Enum):
    """**Component**: The type of the background in a view."""

    GradientDark = 1
    """
    A dark gradient.

    In 3D views it changes depending on the direction of the view.
    """

    GradientBright = 2
    """
    A bright gradient.

    In 3D views it changes depending on the direction of the view.
    """

    SolidColor = 3
    """Simple uniform color."""

    @classmethod
    def auto(cls, val: str | int | BackgroundKind) -> BackgroundKind:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, BackgroundKind):
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


BackgroundKindLike = Union[
    BackgroundKind,
    Literal["GradientBright", "GradientDark", "SolidColor", "gradientbright", "gradientdark", "solidcolor"],
    int,
]
BackgroundKindArrayLike = Union[BackgroundKindLike, Sequence[BackgroundKindLike]]


class BackgroundKindBatch(BaseBatch[BackgroundKindArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.uint8()
    _COMPONENT_NAME: str = "rerun.blueprint.components.BackgroundKind"

    @staticmethod
    def _native_to_pa_array(data: BackgroundKindArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (BackgroundKind, int, str)):
            data = [data]

        pa_data = [BackgroundKind.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
