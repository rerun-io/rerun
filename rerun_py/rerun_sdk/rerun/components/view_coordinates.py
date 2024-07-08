# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/view_coordinates.fbs".

# You can extend this class by creating a "ViewCoordinatesExt" class in "view_coordinates_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)
from .view_coordinates_ext import ViewCoordinatesExt

__all__ = ["ViewCoordinates", "ViewCoordinatesBatch", "ViewCoordinatesType"]


class ViewCoordinates(ViewCoordinatesExt, datatypes.ViewCoordinates, ComponentMixin):
    """
    **Component**: How we interpret the coordinate system of an entity/space.

    For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?

    The three coordinates are always ordered as [x, y, z].

    For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
    down, and the Z axis points forward.

    The following constants are used to represent the different directions:
     * Up = 1
     * Down = 2
     * Right = 3
     * Left = 4
     * Forward = 5
     * Back = 6
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ViewCoordinatesExt in view_coordinates_ext.py

    # Note: there are no fields here because ViewCoordinates delegates to datatypes.ViewCoordinates
    pass


class ViewCoordinatesType(datatypes.ViewCoordinatesType):
    _TYPE_NAME: str = "rerun.components.ViewCoordinates"


class ViewCoordinatesBatch(datatypes.ViewCoordinatesBatch, ComponentBatchMixin):
    _ARROW_TYPE = ViewCoordinatesType()


# This is patched in late to avoid circular dependencies.
ViewCoordinates._BATCH_TYPE = ViewCoordinatesBatch  # type: ignore[assignment]

ViewCoordinatesExt.deferred_patch_class(ViewCoordinates)
