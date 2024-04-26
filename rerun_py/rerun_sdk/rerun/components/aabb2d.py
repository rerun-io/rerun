# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/aabb2d.fbs".

# You can extend this class by creating a "AABB2DExt" class in "aabb2d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["AABB2D", "AABB2DBatch", "AABB2DType"]


class AABB2D(datatypes.AABB2D):
    """**Component**: An Axis-Aligned Bounding Box in 2D space."""

    # You can define your own __init__ function as a member of AABB2DExt in aabb2d_ext.py

    # Note: there are no fields here because AABB2D delegates to datatypes.AABB2D
    pass


class AABB2DType(datatypes.AABB2DType):
    _TYPE_NAME: str = "rerun.components.AABB2D"


class AABB2DBatch(datatypes.AABB2DBatch, ComponentBatchMixin):
    _ARROW_TYPE = AABB2DType()
