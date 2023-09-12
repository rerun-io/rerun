# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/half_sizes2d.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["HalfSizes2D", "HalfSizes2DArray", "HalfSizes2DType"]


class HalfSizes2D(datatypes.Vec2D):
    """
    Half-sizes (extents) of a 2D box along its local axis, starting at its local origin/center.

    The box extends both in negative and positive direction along each axis.
    Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
    """

    # You can define your own __init__ function as a member of HalfSizes2DExt in half_sizes2d_ext.py

    # Note: there are no fields here because HalfSizes2D delegates to datatypes.Vec2D


class HalfSizes2DType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.HalfSizes2D"
    _DELEGATED_EXTENSION_TYPE = datatypes.Vec2DType


class HalfSizes2DArray(BaseDelegatingExtensionArray[datatypes.Vec2DArrayLike]):
    _EXTENSION_NAME = "rerun.components.HalfSizes2D"
    _EXTENSION_TYPE = HalfSizes2DType
    _DELEGATED_ARRAY_TYPE = datatypes.Vec2DArray


HalfSizes2DType._ARRAY_TYPE = HalfSizes2DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(HalfSizes2DType())
