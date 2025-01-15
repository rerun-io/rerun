# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/ellipsoids3d.fbs".

# You can extend this class by creating a "Ellipsoids3DExt" class in "ellipsoids3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .ellipsoids3d_ext import Ellipsoids3DExt

__all__ = ["Ellipsoids3D"]


@define(str=False, repr=False, init=False)
class Ellipsoids3D(Ellipsoids3DExt, Archetype):
    """
    **Archetype**: 3D ellipsoids or spheres.

    This archetype is for ellipsoids or spheres whose size is a key part of the data
    (e.g. a bounding sphere).
    For points whose radii are for the sake of visualization, use [`archetypes.Points3D`][rerun.archetypes.Points3D] instead.

    Note that orienting and placing the ellipsoids/spheres is handled via `[archetypes.InstancePoses3D]`.
    Some of its component are repeated here for convenience.
    If there's more instance poses than half sizes, the last half size will be repeated for the remaining poses.

    Example
    -------
    ### Covariance ellipsoid:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_ellipsoid_simple", spawn=True)

    center = np.array([0, 0, 0])
    sigmas = np.array([5, 3, 1])
    points = np.random.randn(50_000, 3) * sigmas.reshape(1, -1)

    rr.log("points", rr.Points3D(points, radii=0.02, colors=[188, 77, 185]))
    rr.log(
        "ellipsoid",
        rr.Ellipsoids3D(
            centers=[center, center],
            half_sizes=[sigmas, 3 * sigmas],
            colors=[[255, 255, 0], [64, 64, 0]],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/1200w.png">
      <img src="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in ellipsoids3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            half_sizes=None,
            centers=None,
            rotation_axis_angles=None,
            quaternions=None,
            colors=None,
            line_radii=None,
            fill_mode=None,
            labels=None,
            show_labels=None,
            class_ids=None,
        )

    @classmethod
    def _clear(cls) -> Ellipsoids3D:
        """Produce an empty Ellipsoids3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        line_radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Ellipsoids3D:
        """
        Update only some specific fields of a `Ellipsoids3D`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        half_sizes:
            For each ellipsoid, half of its size on its three axes.

            If all components are equal, then it is a sphere with that radius.
        centers:
            Optional center positions of the ellipsoids.

            If not specified, the centers will be at (0, 0, 0).
            Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        colors:
            Optional colors for the ellipsoids.
        line_radii:
            Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
        fill_mode:
            Optionally choose whether the ellipsoids are drawn with lines or solid.
        labels:
            Optional text labels for the ellipsoids.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional class ID for the ellipsoids.

            The class ID provides colors and labels if not specified explicitly.

        """

        kwargs = {
            "half_sizes": half_sizes,
            "centers": centers,
            "rotation_axis_angles": rotation_axis_angles,
            "quaternions": quaternions,
            "colors": colors,
            "line_radii": line_radii,
            "fill_mode": fill_mode,
            "labels": labels,
            "show_labels": show_labels,
            "class_ids": class_ids,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return Ellipsoids3D(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> Ellipsoids3D:
        """Clear all the fields of a `Ellipsoids3D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            half_sizes=[],
            centers=[],
            rotation_axis_angles=[],
            quaternions=[],
            colors=[],
            line_radii=[],
            fill_mode=[],
            labels=[],
            show_labels=[],
            class_ids=[],
        )
        return inst

    half_sizes: components.HalfSize3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.HalfSize3DBatch._converter,  # type: ignore[misc]
    )
    # For each ellipsoid, half of its size on its three axes.
    #
    # If all components are equal, then it is a sphere with that radius.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    centers: components.PoseTranslation3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseTranslation3DBatch._converter,  # type: ignore[misc]
    )
    # Optional center positions of the ellipsoids.
    #
    # If not specified, the centers will be at (0, 0, 0).
    # Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotation_axis_angles: components.PoseRotationAxisAngleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationAxisAngleBatch._converter,  # type: ignore[misc]
    )
    # Rotations via axis + angle.
    #
    # If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    quaternions: components.PoseRotationQuatBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationQuatBatch._converter,  # type: ignore[misc]
    )
    # Rotations via quaternion.
    #
    # If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the ellipsoids.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    line_radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    fill_mode: components.FillModeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.FillModeBatch._converter,  # type: ignore[misc]
    )
    # Optionally choose whether the ellipsoids are drawn with lines or solid.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # Optional text labels for the ellipsoids.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ShowLabelsBatch._converter,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ClassIdBatch._converter,  # type: ignore[misc]
    )
    # Optional class ID for the ellipsoids.
    #
    # The class ID provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
