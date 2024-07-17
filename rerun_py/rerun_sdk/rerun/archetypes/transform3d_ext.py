from __future__ import annotations

from typing import Any

from rerun.components import Scale3D
from rerun.datatypes import (
    Float32Like,
    Mat3x3ArrayLike,
    Quaternion,
    QuaternionArrayLike,
    RotationAxisAngle,
    RotationAxisAngleArrayLike,
    TranslationRotationScale3D,
    Vec3DArrayLike,
)

from ..error_utils import catch_and_log_exceptions


class Transform3DExt:
    """Extension for [Transform3D][rerun.archetypes.Transform3D]."""

    # TODO(#6831): Most parameters should become `ArrayLike`.
    def __init__(
        self: Any,
        *,
        translation: Vec3DArrayLike | None = None,
        rotation: QuaternionArrayLike | RotationAxisAngleArrayLike | None = None,
        rotation_axis_angle: RotationAxisAngleArrayLike | None = None,
        quaternion: QuaternionArrayLike | None = None,
        scale: Vec3DArrayLike | Float32Like | None = None,
        mat3x3: Mat3x3ArrayLike | None = None,
        from_parent: bool | None = None,
        axis_length: Float32Like | None = None,
    ):
        """
        Create a new instance of the Transform3D archetype.

        Parameters
        ----------
        translation:
            3D translation vector.
        rotation:
            3D rotation, either a quaternion or an axis-angle.
            Mutually exclusive with `quaternion` and `rotation_axis_angle`.
        rotation_axis_angle:
            Axis-angle representing rotation.
            Mutually exclusive with `rotation` parameter.
        quaternion:
            Quaternion representing rotation.
            Mutually exclusive with `rotation` parameter.
        scale:
            3D scale.
        mat3x3:
            3x3 matrix representing scale and rotation, applied after translation.
            Not compatible with `rotation` and `scale` parameters.
            TODO(#3559): Support 4x4 and 4x3 matrices.
        from_parent:
             If true, the transform maps from the parent space to the space where the transform was logged.
             Otherwise, the transform maps from the space to its parent.
        axis_length:
            Visual length of the 3 axes.

            The length is interpreted in the local coordinate system of the transform.
            If the transform is scaled, the axes will be scaled accordingly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if from_parent is None:
                from_parent = False

            if rotation is not None:
                if quaternion is not None or rotation_axis_angle is not None:
                    raise ValueError(
                        "`rotation` parameter can't be combined with `quaternion` or `rotation_axis_angle`."
                    )

                is_rotation_axis_angle = False
                try:
                    if isinstance(rotation, RotationAxisAngle):
                        is_rotation_axis_angle = True
                    elif isinstance(rotation[0], RotationAxisAngle):  # type: ignore[index]
                        is_rotation_axis_angle = True
                except Exception:  # Failed to subscript rotation.
                    pass

                if is_rotation_axis_angle:
                    rotation_axis_angle = rotation  # type: ignore[assignment]
                else:
                    try:
                        is_quaternion = False
                        if isinstance(rotation, Quaternion):
                            is_quaternion = True
                        elif isinstance(rotation[0], Quaternion):  # type: ignore[index]
                            is_quaternion = True
                    except Exception:  # Failed to subscript quaternion.
                        pass
                    if not is_quaternion:
                        raise ValueError("Rotation must be compatible with either RotationQuat or RotationAxisAngle")
                    quaternion = rotation  # type: ignore[assignment]

            if scale is not None and (not hasattr(scale, "__len__") or len(scale) == 1):  # type: ignore[arg-type]
                scale = Scale3D(scale)  # type: ignore[arg-type]

            self.__attrs_init__(
                # TODO(#6831): Remove.
                transform=TranslationRotationScale3D(from_parent=from_parent),
                translation=translation,
                rotation_axis_angle=rotation_axis_angle,
                quaternion=quaternion,
                scale=scale,
                mat3x3=mat3x3,
                axis_length=axis_length,
            )
            return
        self.__attrs_clear__()
