# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/zoom_level.fbs".

# You can extend this class by creating a "ZoomLevelExt" class in "zoom_level_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["ZoomLevel", "ZoomLevelBatch", "ZoomLevelType"]


class ZoomLevel(datatypes.Float32, ComponentMixin):
    """**Component**: A zoom level determines how much of the world is visible on a map."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ZoomLevelExt in zoom_level_ext.py

    # Note: there are no fields here because ZoomLevel delegates to datatypes.Float32
    pass


class ZoomLevelType(datatypes.Float32Type):
    _TYPE_NAME: str = "rerun.blueprint.components.ZoomLevel"


class ZoomLevelBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _ARROW_TYPE = ZoomLevelType()


# This is patched in late to avoid circular dependencies.
ZoomLevel._BATCH_TYPE = ZoomLevelBatch  # type: ignore[assignment]
