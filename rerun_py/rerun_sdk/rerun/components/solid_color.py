# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/solid_color.fbs".

# You can extend this class by creating a "SolidColorExt" class in "solid_color_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["SolidColor", "SolidColorBatch", "SolidColorType"]


class SolidColor(datatypes.Rgba32, ComponentMixin):
    """
    **Component**: An RGBA color for the surface of an object.

    In representation and color space, this is identical to [`components.Color`][rerun.components.Color].
    Unlike that component, it is used specifically to request that this color should be
    applied to the entire surface of the object (as opposed to the lines of a wireframe).
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of SolidColorExt in solid_color_ext.py

    # Note: there are no fields here because SolidColor delegates to datatypes.Rgba32
    pass


class SolidColorType(datatypes.Rgba32Type):
    _TYPE_NAME: str = "rerun.components.SolidColor"


class SolidColorBatch(datatypes.Rgba32Batch, ComponentBatchMixin):
    _ARROW_TYPE = SolidColorType()


# This is patched in late to avoid circular dependencies.
SolidColor._BATCH_TYPE = SolidColorBatch  # type: ignore[assignment]
