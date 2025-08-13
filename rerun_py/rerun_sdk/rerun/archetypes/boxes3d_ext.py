from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import components, datatypes


class Boxes3DExt:
    """Extension for [Boxes3D][rerun.archetypes.Boxes3D]."""

    def __init__(
        self: Any,
        *,
        sizes: datatypes.Vec3DArrayLike | None = None,
        mins: datatypes.Vec3DArrayLike | None = None,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        rotations: datatypes.RotationAxisAngleArrayLike | datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Boxes3D archetype.

        Parameters
        ----------
        sizes:
            Full extents in x/y/z. Specify this instead of `half_sizes`
        half_sizes:
            All half-extents that make up the batch of boxes. Specify this instead of `sizes`
        mins:
            Minimum coordinates of the boxes. Specify this instead of `centers`.

            Only valid when used together with either `sizes` or `half_sizes`.
        centers:
            Optional center positions of the boxes.

            If not specified, the centers will be at (0, 0, 0).
            Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotations:
            Backwards compatible parameter for specifying rotations. Tries to infer the type of rotation from the input. Prefer using `quaternions` or `rotation_axis_angles`.
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        fill_mode:
            Optionally choose whether the boxes are drawn with lines or solid.
        labels:
            Optional text labels for the boxes.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional `ClassId`s for the boxes.

            The class ID provides colors and labels if not specified explicitly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if sizes is not None:
                if half_sizes is not None:
                    _send_warning_or_raise("Cannot specify both `sizes` and `half_sizes` at the same time.", 1)

                sizes = np.asarray(sizes, dtype=np.float32)
                half_sizes = sizes / 2.0

            if mins is not None:
                if centers is not None:
                    _send_warning_or_raise("Cannot specify both `mins` and `centers` at the same time.", 1)

                # already converted `sizes` to `half_sizes`
                if half_sizes is None:
                    _send_warning_or_raise("Cannot specify `mins` without `sizes` or `half_sizes`.", 1)
                    half_sizes = np.asarray([1, 1, 1], dtype=np.float32)

                mins = np.asarray(mins, dtype=np.float32)
                half_sizes = np.asarray(half_sizes, dtype=np.float32)
                centers = mins + half_sizes

            if rotations is not None:
                if quaternions is not None or rotation_axis_angles is not None:
                    _send_warning_or_raise(
                        "Cannot specify both `rotations` and `quaternions` or `rotation_axis_angles`.",
                        1,
                    )
                else:
                    try:
                        from ..components import PoseRotationQuatBatch

                        quaternions = PoseRotationQuatBatch(rotations, strict=True).as_arrow_array()  # type: ignore[arg-type]
                        rotation_axis_angles = []
                    except Exception:
                        pass

                    if quaternions is None:
                        try:
                            from ..components import PoseRotationAxisAngleBatch

                            rotation_axis_angles = PoseRotationAxisAngleBatch(rotations, strict=True).as_arrow_array()  # type: ignore[arg-type]
                            quaternions = []
                        except Exception:
                            pass

                    if rotation_axis_angles is None and quaternions is None:
                        _send_warning_or_raise(
                            "Could not infer the type of rotation from the input. Please use `quaternions` or `rotation_axis_angles`.",
                            1,
                        )

            self.__attrs_init__(
                half_sizes=half_sizes,
                centers=centers,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                colors=colors,
                radii=radii,
                fill_mode=fill_mode,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
