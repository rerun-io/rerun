# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/resolution.fbs".

# You can extend this class by creating a "ResolutionExt" class in "resolution_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["Resolution", "ResolutionBatch"]


class Resolution(datatypes.Vec2D, ComponentMixin):
    """
    **Component**: Pixel resolution width & height, e.g. of a camera sensor.

    Typically in integer units, but for some use cases floating point may be used.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ResolutionExt in resolution_ext.py

    # Note: there are no fields here because Resolution delegates to datatypes.Vec2D


class ResolutionBatch(datatypes.Vec2DBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.Resolution")


# This is patched in late to avoid circular dependencies.
Resolution._BATCH_TYPE = ResolutionBatch  # type: ignore[assignment]
