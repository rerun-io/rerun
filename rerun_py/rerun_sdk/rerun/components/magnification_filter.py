# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/magnification_filter.fbs".

# You can extend this class by creating a "MagnificationFilterExt" class in "magnification_filter_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = [
    "MagnificationFilter",
    "MagnificationFilterArrayLike",
    "MagnificationFilterBatch",
    "MagnificationFilterLike",
    "MagnificationFilterType",
]


from enum import Enum


class MagnificationFilter(Enum):
    """**Component**: Filter used when magnifying an image/texture such that a single pixel/texel is displayed as multiple pixels on screen."""

    Nearest = 0
    """
    Show the nearest pixel value.

    This will give a blocky appearance when zooming in.
    Used as default when rendering 2D images.
    """

    Linear = 1
    """
    Linearly interpolate the nearest neighbors, creating a smoother look when zooming in.

    Used as default for mesh rendering.
    """

    @classmethod
    def auto(cls, val: str | int | MagnificationFilter) -> MagnificationFilter:
        """Best-effort converter."""
        if isinstance(val, MagnificationFilter):
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


MagnificationFilterLike = Union[MagnificationFilter, Literal["Linear", "Nearest", "linear", "nearest"], int]
MagnificationFilterArrayLike = Union[MagnificationFilterLike, Sequence[MagnificationFilterLike]]


class MagnificationFilterType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.MagnificationFilter"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class MagnificationFilterBatch(BaseBatch[MagnificationFilterArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = MagnificationFilterType()

    @staticmethod
    def _native_to_pa_array(data: MagnificationFilterArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (MagnificationFilter, int, str)):
            data = [data]

        pa_data = [MagnificationFilter.auto(v).value for v in data]

        return pa.array(pa_data, type=data_type)
