# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/component_column_selector.fbs".

# You can extend this class by creating a "ComponentColumnSelectorExt" class in "component_column_selector_ext.py".

from __future__ import annotations

from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes

__all__ = ["ComponentColumnSelector", "ComponentColumnSelectorBatch", "ComponentColumnSelectorType"]


class ComponentColumnSelector(blueprint_datatypes.ComponentColumnSelector, ComponentMixin):
    """**Component**: Describe a component column to be selected in the dataframe view."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ComponentColumnSelectorExt in component_column_selector_ext.py

    # Note: there are no fields here because ComponentColumnSelector delegates to datatypes.ComponentColumnSelector
    pass


class ComponentColumnSelectorType(blueprint_datatypes.ComponentColumnSelectorType):
    _TYPE_NAME: str = "rerun.blueprint.components.ComponentColumnSelector"


class ComponentColumnSelectorBatch(blueprint_datatypes.ComponentColumnSelectorBatch, ComponentBatchMixin):
    _ARROW_TYPE = ComponentColumnSelectorType()


# This is patched in late to avoid circular dependencies.
ComponentColumnSelector._BATCH_TYPE = ComponentColumnSelectorBatch  # type: ignore[assignment]
