# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/position2d.fbs".

# You can extend this class by creating a "Position2DExt" class in "position2d_ext.py".

from __future__ import annotations

from typing import Any

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Position2D", "Position2DBatch", "Position2DType"]


class Position2D(datatypes.Vec2D):
    """A position in 2D space."""

    def __init__(self: Any, xy: datatypes.Vec2DLike):
        """Create a new instance of the Position2D component."""

        # You can define your own __init__ function as a member of Position2DExt in position2d_ext.py
        self.__attrs_init__(xy=xy)

    # Note: there are no fields here because Position2D delegates to datatypes.Vec2D


class Position2DType(datatypes.Vec2DType):
    _TYPE_NAME: str = "rerun.components.Position2D"


class Position2DBatch(datatypes.Vec2DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Position2DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Position2DType())
