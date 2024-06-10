# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/axis_length.fbs".

# You can extend this class by creating a "AxisLengthExt" class in "axis_length_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["AxisLength", "AxisLengthBatch", "AxisLengthType"]


class AxisLength(datatypes.Float32):
    """**Component**: The length of an axis in local units of the space."""

    # You can define your own __init__ function as a member of AxisLengthExt in axis_length_ext.py

    # Note: there are no fields here because AxisLength delegates to datatypes.Float32
    pass


class AxisLengthType(datatypes.Float32Type):
    _TYPE_NAME: str = "rerun.components.AxisLength"


class AxisLengthBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _ARROW_TYPE = AxisLengthType()
