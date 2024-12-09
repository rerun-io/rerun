# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/length.fbs".

# You can extend this class by creating a "LengthExt" class in "length_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["Length", "LengthBatch"]


class Length(datatypes.Float32, ComponentMixin):
    """
    **Component**: Length, or one-dimensional size.

    Measured in its local coordinate system; consult the archetype in use to determine which
    axis or part of the entity this is the length of.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of LengthExt in length_ext.py

    # Note: there are no fields here because Length delegates to datatypes.Float32
    pass


class LengthBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.Length")


# This is patched in late to avoid circular dependencies.
Length._BATCH_TYPE = LengthBatch  # type: ignore[assignment]
