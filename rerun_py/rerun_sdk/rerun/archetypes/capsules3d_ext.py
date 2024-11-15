from __future__ import annotations

from typing import Any

from .. import datatypes
from ..error_utils import catch_and_log_exceptions


class Capsules3DExt:
    """Extension for [Capsules3D][rerun.archetypes.Capsules3D]."""

    def __init__(
        self: Any,
        *,
        lengths: datatypes.Float32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        translations: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Capsules3D archetype.

        Parameters
        ----------
        lengths:
            All lengths of the capsules.
        radii:
            All radii of the capsules.
        translations:
            Optional translations of the capsules.

            If not specified, one end of each capsule will be at (0, 0, 0).
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
            Optional text labels for the capsules.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional `ClassId`s for the capsules.

            The class ID provides colors and labels if not specified explicitly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                lengths=lengths,
                radii=radii,
                translations=translations,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
