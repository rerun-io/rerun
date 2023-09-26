# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/half_sizes2d.fbs".

# You can extend this class by creating a "HalfSizes2DExt" class in "half_sizes2d_ext.py".

from __future__ import annotations

from typing import Any

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["HalfSizes2D", "HalfSizes2DBatch", "HalfSizes2DType"]


class HalfSizes2D(datatypes.Vec2D):
    """
    Half-sizes (extents) of a 2D box along its local axis, starting at its local origin/center.

    The box extends both in negative and positive direction along each axis.
    Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
    """

    def __init__(self: Any, xy: datatypes.Vec2DLike):
        """Create a new instance of the HalfSizes2D component."""
        # You can define your own __init__ function as a member of HalfSizes2DExt in half_sizes2d_ext.py
        self.__attrs_init__(xy=xy)

    # Note: there are no fields here because HalfSizes2D delegates to datatypes.Vec2D


class HalfSizes2DType(datatypes.Vec2DType):
    _TYPE_NAME: str = "rerun.components.HalfSizes2D"


class HalfSizes2DBatch(datatypes.Vec2DBatch, ComponentBatchMixin):
    _ARROW_TYPE = HalfSizes2DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(HalfSizes2DType())
