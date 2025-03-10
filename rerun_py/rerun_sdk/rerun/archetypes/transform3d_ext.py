from __future__ import annotations

from typing import Any

from rerun.components import Scale3D, TransformRelation, TransformRelationLike
from rerun.datatypes import (
    Float32Like,
    Mat3x3Like,
    Quaternion,
    QuaternionLike,
    RotationAxisAngle,
    RotationAxisAngleLike,
    Vec3DLike,
)

from ..error_utils import catch_and_log_exceptions


class Transform3DExt:
    """Extension for [Transform3D][rerun.archetypes.Transform3D]."""

    def __init__(
        self: Any,
        *,
        clear: bool = True,
        translation: Vec3DLike | None = None,
        rotation: QuaternionLike | RotationAxisAngleLike | None = None,
        rotation_axis_angle: RotationAxisAngleLike | None = None,
        quaternion: QuaternionLike | None = None,
        scale: Vec3DLike | Float32Like | None = None,
        mat3x3: Mat3x3Like | None = None,
        from_parent: bool | None = None,
        relation: TransformRelationLike | None = None,
        axis_length: Float32Like | None = None,
    ) -> None:
        """
        Create a new instance of the Transform3D archetype.

        Parameters
        ----------
        clear:
             If true (the default), all unspecified fields will be explicitly cleared.
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
            Deprecated in favor of `relation=rerun.TransformRelation.ChildFromParent`.

            Mutually exclusive with `relation`.
        relation:
            Allows to explicitly specify the transform's relationship with the parent entity.
            Otherwise, the transform maps from the space to its parent.

            Mutually exclusive with `from_parent`.
        axis_length:
            Visual length of the 3 axes.

            The length is interpreted in the local coordinate system of the transform.
            If the transform is scaled, the axes will be scaled accordingly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if rotation is not None:
                if quaternion is not None or rotation_axis_angle is not None:
                    raise ValueError(
                        "`rotation` parameter can't be combined with `quaternion` or `rotation_axis_angle`.",
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

            if from_parent is not None:
                import warnings

                warnings.warn(
                    message=(
                        "`from_parent` is deprecated as an argument to `Transform3D`; prefer `relation=rerun.TransformRelation.ChildFromParent` instead"
                    ),
                    category=DeprecationWarning,
                    stacklevel=2,
                )

                if relation is not None:
                    raise ValueError("`from_parent` and `relation` parameters are mutually exclusive.")
                if from_parent:
                    relation = TransformRelation.ChildFromParent

            if clear:
                self.__attrs_init__(
                    translation=translation if translation is not None else [],
                    rotation_axis_angle=rotation_axis_angle if rotation_axis_angle is not None else [],
                    quaternion=quaternion if quaternion is not None else [],
                    scale=scale if scale is not None else [],
                    mat3x3=mat3x3 if mat3x3 is not None else [],
                    relation=relation if relation is not None else [],
                    axis_length=axis_length if axis_length is not None else [],
                )
            else:
                self.__attrs_init__(
                    translation=translation,
                    rotation_axis_angle=rotation_axis_angle,
                    quaternion=quaternion,
                    scale=scale,
                    mat3x3=mat3x3,
                    relation=relation,
                    axis_length=axis_length,
                )
            return
        self.__attrs_clear__()
