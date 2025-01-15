# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/capsules3d.fbs".

# You can extend this class by creating a "Capsules3DExt" class in "capsules3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .capsules3d_ext import Capsules3DExt

__all__ = ["Capsules3D"]


@define(str=False, repr=False, init=False)
class Capsules3D(Capsules3DExt, Archetype):
    """
    **Archetype**: 3D capsules; cylinders with hemispherical caps.

    Capsules are defined by two endpoints (the centers of their end cap spheres), which are located
    at (0, 0, 0) and (0, 0, length), that is, extending along the positive direction of the Z axis.
    Capsules in other orientations may be produced by applying a rotation to the entity or
    instances.

    Example
    -------
    ### Batch of capsules:
    ```python
    import rerun as rr

    rr.init("rerun_example_capsule3d_batch", spawn=True)

    rr.log(
        "capsules",
        rr.Capsules3D(
            lengths=[0.0, 2.0, 4.0, 6.0, 8.0],
            radii=[1.0, 0.5, 0.5, 0.5, 1.0],
            colors=[
                (255, 0, 0),
                (188, 188, 0),
                (0, 255, 0),
                (0, 188, 188),
                (0, 0, 255),
            ],
            translations=[
                (0.0, 0.0, 0.0),
                (2.0, 0.0, 0.0),
                (4.0, 0.0, 0.0),
                (6.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
            rotation_axis_angles=[
                rr.RotationAxisAngle(
                    [1.0, 0.0, 0.0],
                    rr.Angle(deg=float(i) * -22.5),
                )
                for i in range(0, 5)
            ],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/1200w.png">
      <img src="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in capsules3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            lengths=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
            translations=None,  # type: ignore[arg-type]
            rotation_axis_angles=None,  # type: ignore[arg-type]
            quaternions=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            show_labels=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Capsules3D:
        """Produce an empty Capsules3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        lengths: datatypes.Float32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        translations: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Capsules3D:
        """
        Update only some specific fields of a `Capsules3D`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        lengths:
            Lengths of the capsules, defined as the distance between the centers of the endcaps.
        radii:
            Radii of the capsules.
        translations:
            Optional translations of the capsules.

            If not specified, one end of each capsule will be at (0, 0, 0).
            Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        colors:
            Optional colors for the capsules.
        labels:
            Optional text labels for the capsules, which will be located at their centers.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional class ID for the ellipsoids.

            The class ID provides colors and labels if not specified explicitly.

        """

        kwargs = {
            "lengths": lengths,
            "radii": radii,
            "translations": translations,
            "rotation_axis_angles": rotation_axis_angles,
            "quaternions": quaternions,
            "colors": colors,
            "labels": labels,
            "show_labels": show_labels,
            "class_ids": class_ids,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return Capsules3D(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> Capsules3D:
        """Clear all the fields of a `Capsules3D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            lengths=[],  # type: ignore[arg-type]
            radii=[],  # type: ignore[arg-type]
            translations=[],  # type: ignore[arg-type]
            rotation_axis_angles=[],  # type: ignore[arg-type]
            quaternions=[],  # type: ignore[arg-type]
            colors=[],  # type: ignore[arg-type]
            labels=[],  # type: ignore[arg-type]
            show_labels=[],  # type: ignore[arg-type]
            class_ids=[],  # type: ignore[arg-type]
        )
        return inst

    lengths: components.LengthBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.LengthBatch._optional,  # type: ignore[misc]
    )
    # Lengths of the capsules, defined as the distance between the centers of the endcaps.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Radii of the capsules.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    translations: components.PoseTranslation3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.PoseTranslation3DBatch._optional,  # type: ignore[misc]
    )
    # Optional translations of the capsules.
    #
    # If not specified, one end of each capsule will be at (0, 0, 0).
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
    # If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
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
    # If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
    # Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the capsules.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the capsules, which will be located at their centers.
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
    # Optional class ID for the ellipsoids.
    #
    # The class ID provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
