# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/origin2d.fbs".

# You can extend this class by creating a "Origin2DExt" class in "origin2d_ext.py".

from __future__ import annotations

from typing import Any

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Origin2D", "Origin2DBatch", "Origin2DType"]


class Origin2D(datatypes.Vec2D):
    """A point of origin in 2D space."""

    def __init__(self: Any, xy: datatypes.Vec2DLike):
        """Create a new instance of the Origin2D component."""
        # You can define your own __init__ function as a member of Origin2DExt in origin2d_ext.py
        self.__attrs_init__(xy=xy)

    # Note: there are no fields here because Origin2D delegates to datatypes.Vec2D


class Origin2DType(datatypes.Vec2DType):
    _TYPE_NAME: str = "rerun.components.Origin2D"


class Origin2DBatch(datatypes.Vec2DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Origin2DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Origin2DType())
