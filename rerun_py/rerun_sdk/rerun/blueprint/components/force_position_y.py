# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/force_position_y.fbs".

# You can extend this class by creating a "ForcePositionYExt" class in "force_position_y_ext.py".

from __future__ import annotations

from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes

__all__ = ["ForcePositionY", "ForcePositionYBatch"]


class ForcePositionY(blueprint_datatypes.ForcePositionY, ComponentMixin):
    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ForcePositionYExt in force_position_y_ext.py

    # Note: there are no fields here because ForcePositionY delegates to datatypes.ForcePositionY
    pass


class ForcePositionYBatch(blueprint_datatypes.ForcePositionYBatch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.blueprint.components.ForcePositionY"


# This is patched in late to avoid circular dependencies.
ForcePositionY._BATCH_TYPE = ForcePositionYBatch  # type: ignore[assignment]
