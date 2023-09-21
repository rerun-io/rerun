# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/view_coordinates.fbs".

# You can extend this class by creating a "ViewCoordinatesExt" class in "view_coordinates_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .view_coordinates_ext import ViewCoordinatesExt

__all__ = ["ViewCoordinates"]


@define(str=False, repr=False)
class ViewCoordinates(ViewCoordinatesExt, Archetype):
    """
    How we interpret the coordinate system of an entity/space.

    Example
    -------
    ```python

    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_view_coordinate", spawn=True)

    rr2.log("/", rr2.ViewCoordinates.ULB)
    rr2.log(
        "xyz",
        rr2.Arrows3D(
            vectors=[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        ),
    )
    ```
    """

    # You can define your own __init__ function as a member of ViewCoordinatesExt in view_coordinates_ext.py

    coordinates: components.ViewCoordinatesArray = field(
        metadata={"component": "required"},
        converter=components.ViewCoordinatesArray.from_similar,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
