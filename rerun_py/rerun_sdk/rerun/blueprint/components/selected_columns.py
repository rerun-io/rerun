# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/selected_columns.fbs".

# You can extend this class by creating a "SelectedColumnsExt" class in "selected_columns_ext.py".

from __future__ import annotations

from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes

__all__ = ["SelectedColumns", "SelectedColumnsBatch"]


class SelectedColumns(blueprint_datatypes.SelectedColumns, ComponentMixin):
    """**Component**: Describe a component column to be selected in the dataframe view."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of SelectedColumnsExt in selected_columns_ext.py

    # Note: there are no fields here because SelectedColumns delegates to datatypes.SelectedColumns


class SelectedColumnsBatch(blueprint_datatypes.SelectedColumnsBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.SelectedColumns")


# This is patched in late to avoid circular dependencies.
SelectedColumns._BATCH_TYPE = SelectedColumnsBatch  # type: ignore[assignment]
