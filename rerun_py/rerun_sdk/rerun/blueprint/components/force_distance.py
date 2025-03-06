# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/force_distance.fbs".

# You can extend this class by creating a "ForceDistanceExt" class in "force_distance_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["ForceDistance", "ForceDistanceBatch"]


class ForceDistance(datatypes.Float64, ComponentMixin):
    """
    **Component**: The target distance between two nodes.

    This is helpful to scale the layout, for example if long labels are involved.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ForceDistanceExt in force_distance_ext.py

    # Note: there are no fields here because ForceDistance delegates to datatypes.Float64


class ForceDistanceBatch(datatypes.Float64Batch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.ForceDistance")


# This is patched in late to avoid circular dependencies.
ForceDistance._BATCH_TYPE = ForceDistanceBatch  # type: ignore[assignment]
