# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/auto_views.fbs".

# You can extend this class by creating a "AutoViewsExt" class in "auto_views_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["AutoViews", "AutoViewsBatch"]


class AutoViews(datatypes.Bool, ComponentMixin):
    """**Component**: Whether or not views should be created automatically."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of AutoViewsExt in auto_views_ext.py

    # Note: there are no fields here because AutoViews delegates to datatypes.Bool
    pass


class AutoViewsBatch(datatypes.BoolBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.AutoViews")


# This is patched in late to avoid circular dependencies.
AutoViews._BATCH_TYPE = AutoViewsBatch  # type: ignore[assignment]
