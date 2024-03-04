# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/space_view_class.fbs".

# You can extend this class by creating a "SpaceViewClassExt" class in "space_view_class_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import ComponentBatchMixin

__all__ = ["SpaceViewClass", "SpaceViewClassBatch", "SpaceViewClassType"]


class SpaceViewClass(datatypes.Utf8):
    """**Component**: The class of a `SpaceView`."""

    # You can define your own __init__ function as a member of SpaceViewClassExt in space_view_class_ext.py

    # Note: there are no fields here because SpaceViewClass delegates to datatypes.Utf8
    pass


class SpaceViewClassType(datatypes.Utf8Type):
    _TYPE_NAME: str = "rerun.blueprint.components.SpaceViewClass"


class SpaceViewClassBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _ARROW_TYPE = SpaceViewClassType()
