# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/vector2d.fbs".

# You can extend this class by creating a "Vector2DExt" class in "vector2d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["Vector2D", "Vector2DBatch"]


class Vector2D(datatypes.Vec2D, ComponentMixin):
    """**Component**: A vector in 2D space."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of Vector2DExt in vector2d_ext.py

    # Note: there are no fields here because Vector2D delegates to datatypes.Vec2D


class Vector2DBatch(datatypes.Vec2DBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.Vector2D")


# This is patched in late to avoid circular dependencies.
Vector2D._BATCH_TYPE = Vector2DBatch  # type: ignore[assignment]
