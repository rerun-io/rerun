# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/ui_radius.fbs".

# You can extend this class by creating a "UiRadiusExt" class in "ui_radius_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["UiRadius", "UiRadiusBatch"]


class UiRadius(datatypes.Float32, ComponentMixin):
    """**Component**: Like `Radius`, but in always in ui units."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of UiRadiusExt in ui_radius_ext.py

    # Note: there are no fields here because UiRadius delegates to datatypes.Float32
    pass


class UiRadiusBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.blueprint.components.UiRadius"


# This is patched in late to avoid circular dependencies.
UiRadius._BATCH_TYPE = UiRadiusBatch  # type: ignore[assignment]
