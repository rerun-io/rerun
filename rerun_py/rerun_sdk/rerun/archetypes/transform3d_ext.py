from __future__ import annotations

from typing import Any

from rerun.components import Scale3D
from rerun.datatypes import (
    Float32Like,
    Mat3x3ArrayLike,
    Rotation3DLike,
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
        rotation: Rotation3DLike | None = None,  # TODO(#6831): Should allow arrays.
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
            3D rotation.
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

            if scale is not None and (not hasattr(scale, "__len__") or len(scale) == 1):  # type: ignore[arg-type]
                print("reached scalar case")
                scale = Scale3D(scale)  # type: ignore[arg-type]
                print(scale)

            self.__attrs_init__(
                # TODO(#6831): Remove.
                transform=TranslationRotationScale3D(
                    rotation=rotation,
                    from_parent=from_parent,
                ),
                mat3x3=mat3x3,
                scale=scale,
                translation=translation,
                axis_length=axis_length,
            )
            return
        self.__attrs_clear__()
