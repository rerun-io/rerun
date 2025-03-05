# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/filter_is_not_null.fbs".

# You can extend this class by creating a "FilterIsNotNullExt" class in "filter_is_not_null_ext.py".

from __future__ import annotations

from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes

__all__ = ["FilterIsNotNull", "FilterIsNotNullBatch"]


class FilterIsNotNull(blueprint_datatypes.FilterIsNotNull, ComponentMixin):
    """**Component**: Configuration for the filter is not null feature of the dataframe view."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of FilterIsNotNullExt in filter_is_not_null_ext.py

    # Note: there are no fields here because FilterIsNotNull delegates to datatypes.FilterIsNotNull


class FilterIsNotNullBatch(blueprint_datatypes.FilterIsNotNullBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.FilterIsNotNull")


# This is patched in late to avoid circular dependencies.
FilterIsNotNull._BATCH_TYPE = FilterIsNotNullBatch  # type: ignore[assignment]
