# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/background_kind.fbs".

# You can extend this class by creating a "BackgroundKindExt" class in "background_kind_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = [
    "BackgroundKind",
    "BackgroundKindArrayLike",
    "BackgroundKindBatch",
    "BackgroundKindLike",
    "BackgroundKindType",
]


from enum import Enum


class BackgroundKind(Enum):
    """**Component**: The type of the background in a view."""

    GradientDark = 0
    """
    A dark gradient.

    In 3D views it changes depending on the direction of the view.
    """

    GradientBright = 1
    """
    A bright gradient.

    In 3D views it changes depending on the direction of the view.
    """

    SolidColor = 2
    """Simple uniform color."""

    def __str__(self) -> str:
        """Returns the variant name."""
        if self == BackgroundKind.GradientDark:
            return "GradientDark"
        elif self == BackgroundKind.GradientBright:
            return "GradientBright"
        elif self == BackgroundKind.SolidColor:
            return "SolidColor"
        else:
            raise ValueError("Unknown enum variant")


BackgroundKindLike = Union[
    BackgroundKind,
    Literal["GradientBright", "GradientDark", "SolidColor", "gradientbright", "gradientdark", "solidcolor"],
]
BackgroundKindArrayLike = Union[BackgroundKindLike, Sequence[BackgroundKindLike]]


class BackgroundKindType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.BackgroundKind"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class BackgroundKindBatch(BaseBatch[BackgroundKindArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = BackgroundKindType()

    @staticmethod
    def _native_to_pa_array(data: BackgroundKindArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (BackgroundKind, int, str)):
            data = [data]

        data = [BackgroundKind(v) if isinstance(v, int) else v for v in data]
        data = [BackgroundKind[v.upper()] if isinstance(v, str) else v for v in data]
        pa_data = [v.value for v in data]

        return pa.array(pa_data, type=data_type)
