# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/clipping_plane.fbs".

# You can extend this class by creating a "ClippingPlaneExt" class in "clipping_plane_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["ClippingPlane", "ClippingPlaneBatch"]


class ClippingPlane(datatypes.Float32, ComponentMixin):
    """**Component**: Distance to the clipping plane in used for `Spatial2DView`."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ClippingPlaneExt in clipping_plane_ext.py

    # Note: there are no fields here because ClippingPlane delegates to datatypes.Float32
    pass


class ClippingPlaneBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.blueprint.components.ClippingPlane"


# This is patched in late to avoid circular dependencies.
ClippingPlane._BATCH_TYPE = ClippingPlaneBatch  # type: ignore[assignment]