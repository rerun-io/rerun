# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/space_view_maximized.fbs".

# You can extend this class by creating a "SpaceViewMaximizedExt" class in "space_view_maximized_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["SpaceViewMaximized", "SpaceViewMaximizedBatch"]


class SpaceViewMaximized(datatypes.Uuid, ComponentMixin):
    """**Component**: Whether a view is maximized."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of SpaceViewMaximizedExt in space_view_maximized_ext.py

    # Note: there are no fields here because SpaceViewMaximized delegates to datatypes.Uuid
    pass


class SpaceViewMaximizedBatch(datatypes.UuidBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.SpaceViewMaximized")


# This is patched in late to avoid circular dependencies.
SpaceViewMaximized._BATCH_TYPE = SpaceViewMaximizedBatch  # type: ignore[assignment]
