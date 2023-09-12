# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/point2d.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["Point2D", "Point2DArray", "Point2DType"]


class Point2D(datatypes.Vec2D):
    """A point in 2D space."""

    # You can define your own __init__ function as a member of Point2DExt in point2d_ext.py

    # Note: there are no fields here because Point2D delegates to datatypes.Vec2D


class Point2DType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.point2d"
    _DELEGATED_EXTENSION_TYPE = datatypes.Vec2DType


class Point2DArray(BaseDelegatingExtensionArray[datatypes.Vec2DArrayLike]):
    _EXTENSION_NAME = "rerun.point2d"
    _EXTENSION_TYPE = Point2DType
    _DELEGATED_ARRAY_TYPE = datatypes.Vec2DArray


Point2DType._ARRAY_TYPE = Point2DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Point2DType())
