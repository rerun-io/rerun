# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/space_view_origin.fbs".

# You can extend this class by creating a "SpaceViewOriginExt" class in "space_view_origin_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import ComponentBatchMixin

__all__ = ["SpaceViewOrigin", "SpaceViewOriginBatch", "SpaceViewOriginType"]


class SpaceViewOrigin(datatypes.EntityPath):
    """**Component**: The origin of a `SpaceView`."""

    # You can define your own __init__ function as a member of SpaceViewOriginExt in space_view_origin_ext.py

    # Note: there are no fields here because SpaceViewOrigin delegates to datatypes.EntityPath
    pass


class SpaceViewOriginType(datatypes.EntityPathType):
    _TYPE_NAME: str = "rerun.blueprint.components.SpaceViewOrigin"


class SpaceViewOriginBatch(datatypes.EntityPathBatch, ComponentBatchMixin):
    _ARROW_TYPE = SpaceViewOriginType()
