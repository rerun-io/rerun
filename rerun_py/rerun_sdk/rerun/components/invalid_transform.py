# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/invalid_transform.fbs".

# You can extend this class by creating a "InvalidTransformExt" class in "invalid_transform_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["InvalidTransform", "InvalidTransformBatch"]


class InvalidTransform(datatypes.Bool, ComponentMixin):
    """
    **Component**: Flags the transform at its entity path as invalid.

    Specifies that the entity path at which this is logged is spatially disconnected from its parent,
    making it impossible to transform the entity path into its parent's space and vice versa.
    This can be useful for instance to express temporily unknown transforms.

    Note that by default all transforms are considered valid.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of InvalidTransformExt in invalid_transform_ext.py

    # Note: there are no fields here because InvalidTransform delegates to datatypes.Bool
    pass


class InvalidTransformBatch(datatypes.BoolBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.InvalidTransform")


# This is patched in late to avoid circular dependencies.
InvalidTransform._BATCH_TYPE = InvalidTransformBatch  # type: ignore[assignment]
