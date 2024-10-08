# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/boxes3d.fbs".

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
    **Archetype**: 3D boxes with half-extents and optional center, rotations, colors etc.

    Note that orienting and placing the box is handled via `[archetypes.InstancePoses3D]`.
    Some of its component are repeated here for convenience.
    If there's more instance poses than half sizes, the last half size will be repeated for the remaining poses.

    Example
    -------
    ### Batch of 3D boxes:
    ```python
    import rerun as rr

    rr.init("rerun_example_box3d_batch", spawn=True)

    rr.log(
        "batch",
        rr.Boxes3D(
            centers=[[2, 0, 0], [-2, 0, 0], [0, 0, 2]],
            half_sizes=[[2.0, 2.0, 1.0], [1.0, 1.0, 0.5], [2.0, 0.5, 1.0]],
            quaternions=[
                rr.Quaternion.identity(),
                rr.Quaternion(xyzw=[0.0, 0.0, 0.382683, 0.923880]),  # 45 degrees around Z
            ],
            radii=0.025,
            colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
            fill_mode="solid",
            labels=["red", "green", "blue"],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/1200w.png">
      <img src="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in boxes3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            half_sizes=None,  # type: ignore[arg-type]
            centers=None,  # type: ignore[arg-type]
            rotation_axis_angles=None,  # type: ignore[arg-type]
            quaternions=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
            fill_mode=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            show_labels=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Boxes3D:
        """Produce an empty Boxes3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    half_sizes: components.HalfSize3DBatch = field(
        metadata={"component": "required"},
        converter=components.HalfSize3DBatch._required,  # type: ignore[misc]
    )
    # All half-extents that make up the batch of boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    centers: components.PoseTranslation3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.PoseTranslation3DBatch._optional,  # type: ignore[misc]
    )
    # Optional center positions of the boxes.
    #
    # If not specified, the centers will be at (0, 0, 0).
    # Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotation_axis_angles: components.PoseRotationAxisAngleBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.PoseRotationAxisAngleBatch._optional,  # type: ignore[misc]
    )
    # Rotations via axis + angle.
    #
    # If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    quaternions: components.PoseRotationQuatBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.PoseRotationQuatBatch._optional,  # type: ignore[misc]
    )
    # Rotations via quaternion.
    #
    # If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for the lines that make up the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    fill_mode: components.FillModeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.FillModeBatch._optional,  # type: ignore[misc]
    )
    # Optionally choose whether the boxes are drawn with lines or solid.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the boxes.
    #
    # If there's a single label present, it will be placed at the center of the entity.
    # Otherwise, each instance will have its own label.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ShowLabelsBatch._optional,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
