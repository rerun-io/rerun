# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/position3d.fbs".

# You can extend this class by creating a "Position3DExt" class in "position3d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["Position3D", "Position3DBatch", "Position3DType"]


class Position3D(datatypes.Vec3D, ComponentMixin):
    """**Component**: A position in 3D space."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of Position3DExt in position3d_ext.py

    # Note: there are no fields here because Position3D delegates to datatypes.Vec3D
    pass


class Position3DType(datatypes.Vec3DType):
    _TYPE_NAME: str = "rerun.components.Position3D"


class Position3DBatch(datatypes.Vec3DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Position3DType()


# This is patched in late to avoid circular dependencies.
Position3D._BATCH_TYPE = Position3DBatch  # type: ignore[assignment]
