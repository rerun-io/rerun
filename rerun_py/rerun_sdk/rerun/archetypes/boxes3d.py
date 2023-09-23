# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/boxes3d.fbs".

# You can extend this class by creating a "Boxes3DExt" class in "boxes3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .boxes3d_ext import Boxes3DExt

__all__ = ["Boxes3D"]


@define(str=False, repr=False, init=False)
class Boxes3D(Boxes3DExt, Archetype):
    """
    A batch of 3d boxes with half-extents and optional center, rotations, rotations, colors etc.

    Examples
    --------
    Simple 3D boxes:
    ```python
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_box3d_simple", spawn=True)

    rr2.log("simple", rr2.Boxes3D(half_sizes=[2.0, 2.0, 1.0]))
    ```

    Batch of 3D boxes:
    ```python
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_box3d_batch", spawn=True)

    rr2.log(
        "batch",
        rr2.Boxes3D(
            centers=[[2, 0, 0], [-2, 0, 0], [0, 0, 2]],
            half_sizes=[[2.0, 2.0, 1.0], [1.0, 1.0, 0.5], [2.0, 0.5, 1.0]],
            rotations=[
                rr2.cmp.Rotation3D.identity(),
                rr2.dt.Quaternion(xyzw=[0.0, 0.0, 0.382683, 0.923880]),  # 45 degrees around Z
                rr2.dt.RotationAxisAngle(axis=[0, 1, 0], angle=rr2.dt.Angle(deg=30)),
            ],
            radii=0.025,
            colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
            labels=["red", "green", "blue"],
        ),
    )
    ```
    """

    # __init__ can be found in boxes3d_ext.py

    half_sizes: components.HalfSizes3DArray = field(
        metadata={"component": "required"},
        converter=components.HalfSizes3DArray.from_similar,  # type: ignore[misc]
    )
    """
    All half-extents that make up the batch of boxes.
    """

    centers: components.Position3DArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Position3DArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional center positions of the boxes.
    """

    rotations: components.Rotation3DArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Rotation3DArray.optional_from_similar,  # type: ignore[misc]
    )
    colors: components.ColorArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional colors for the boxes.
    """

    radii: components.RadiusArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional radii for the lines that make up the boxes.
    """

    labels: components.TextArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional text labels for the boxes.
    """

    class_ids: components.ClassIdArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional `ClassId`s for the boxes.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual boxes in the batch.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__


if hasattr(Boxes3DExt, "deferred_patch_class"):
    Boxes3DExt.deferred_patch_class(Boxes3D)
