# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/show_labels.fbs".

# You can extend this class by creating a "ShowLabelsExt" class in "show_labels_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["ShowLabels", "ShowLabelsBatch"]


class ShowLabels(datatypes.Bool, ComponentMixin):
    """
    **Component**: Whether the entity's [`components.Text`][rerun.components.Text] label is shown.

    The main purpose of this component existing separately from the labels themselves
    is to be overridden when desired, to allow hiding and showing from the viewer and
    blueprints.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ShowLabelsExt in show_labels_ext.py

    # Note: there are no fields here because ShowLabels delegates to datatypes.Bool


class ShowLabelsBatch(datatypes.BoolBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.ShowLabels")


# This is patched in late to avoid circular dependencies.
ShowLabels._BATCH_TYPE = ShowLabelsBatch  # type: ignore[assignment]
