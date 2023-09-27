# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/position3d.fbs".

# You can extend this class by creating a "Position3DExt" class in "position3d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Position3D", "Position3DBatch", "Position3DType"]


class Position3D(datatypes.Vec3D):
    """A position in 3D space."""

    # Note: there are no fields here because Position3D delegates to datatypes.Vec3D
    pass


class Position3DType(datatypes.Vec3DType):
    _TYPE_NAME: str = "rerun.components.Position3D"


class Position3DBatch(datatypes.Vec3DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Position3DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Position3DType())
