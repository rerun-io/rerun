# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/interactive.fbs".

# You can extend this class by creating a "InteractiveExt" class in "interactive_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["Interactive", "InteractiveBatch"]


class Interactive(datatypes.Bool, ComponentMixin):
    """
    **Component**: Whether the entity can be interacted with.

    Non interactive components are still visible, but mouse interactions in the view are disabled.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of InteractiveExt in interactive_ext.py

    # Note: there are no fields here because Interactive delegates to datatypes.Bool
    pass


class InteractiveBatch(datatypes.BoolBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.Interactive")


# This is patched in late to avoid circular dependencies.
Interactive._BATCH_TYPE = InteractiveBatch  # type: ignore[assignment]
