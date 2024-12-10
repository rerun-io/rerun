# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/near_clip_plane.fbs".

# You can extend this class by creating a "NearClipPlaneExt" class in "near_clip_plane_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["NearClipPlane", "NearClipPlaneBatch"]


class NearClipPlane(datatypes.Float32, ComponentMixin):
    """**Component**: Distance to the near clip plane used for `Spatial2DView`."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of NearClipPlaneExt in near_clip_plane_ext.py

    # Note: there are no fields here because NearClipPlane delegates to datatypes.Float32
    pass


class NearClipPlaneBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.NearClipPlane")


# This is patched in late to avoid circular dependencies.
NearClipPlane._BATCH_TYPE = NearClipPlaneBatch  # type: ignore[assignment]
