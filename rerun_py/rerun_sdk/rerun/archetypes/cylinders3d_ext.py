from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import components, datatypes


class Cylinders3DExt:
    """Extension for [Cylinders3D][rerun.archetypes.Cylinders3D]."""

    def __init__(
        self: Any,
        *,
        lengths: datatypes.Float32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        line_radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Cylinders3D archetype.

        Parameters
        ----------
        lengths:
            All lengths of the cylinders.
        radii:
            All radii of the cylinders.
        centers:
            Optional centers of the cylinders.

            If not specified, each cylinder will be centered at (0, 0, 0).
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the cylinders align with the +Z axis of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the cylinders align with the +Z axis of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        colors:
            Optional colors for the cylinders.
        line_radii:
            Optional radii for the lines that make up the cylinders.
        fill_mode:
            Optionally choose whether the cylinders are drawn with lines or solid.
        labels:
            Optional text labels for the cylinders.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional `ClassId`s for the cylinders.

            The class ID provides colors and labels if not specified explicitly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                lengths=lengths,
                radii=radii,
                centers=centers,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                colors=colors,
                line_radii=line_radii,
                fill_mode=fill_mode,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
