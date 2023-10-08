from __future__ import annotations

from typing import Any

from rerun.datatypes.mat3x3 import Mat3x3Like
from rerun.datatypes.rotation3d import Rotation3DLike
from rerun.datatypes.scale3d import Scale3DLike
from rerun.datatypes.transform3d import Transform3DLike
from rerun.datatypes.translation_and_mat3x3 import TranslationAndMat3x3
from rerun.datatypes.translation_rotation_scale3d import TranslationRotationScale3D
from rerun.datatypes.vec3d import Vec3DLike

from ..error_utils import catch_and_log_exceptions


class Transform3DExt:
    """Extension for [Transform3D][rerun.archetypes.Transform3D]."""

    def __init__(
        self: Any,
        transform: Transform3DLike | None = None,
        *,
        translation: Vec3DLike | None = None,
        rotation: Rotation3DLike | None = None,
        scale: Scale3DLike | None = None,
        mat3x3: Mat3x3Like | None = None,
        from_parent: bool = False,
    ):
        """
        Create a new instance of the Transform3D archetype.

        Parameters
        ----------
        transform:
            Transform using an existing Transform3D datatype object.
            If not provided, none of the other named parameters must be set.
        translation:
            3D translation vector, applied last.
            Not compatible with `transform`.
        rotation:
            3D rotation, applied second.
            Not compatible with `transform` and `mat3x3` parameters.
        scale:
            3D scale, applied last.
            Not compatible with `transform` and `mat3x3` parameters.
        mat3x3:
            3x3 matrix representing scale and rotation, applied after translation.
            Not compatible with `rotation` and `scale` parameters.
            TODO(#3559): Support 4x4 and 4x3 matrices.
        from_parent:
             If true, the transform maps from the parent space to the space where the transform was logged.
             Otherwise, the transform maps from the space to its parent.
        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if transform is not None:
                if translation is not None or rotation is not None or scale is not None or mat3x3 is not None:
                    raise ValueError("If a transform is given, none of the other parameters can be set.")
                self.__attrs_init__(transform=transform)
            else:
                if rotation is not None and mat3x3 is not None:
                    raise ValueError("Rotation and mat3x3 parameters are mutually exclusive.")
                if scale is not None and mat3x3 is not None:
                    raise ValueError("Scale and mat3x3 parameters are mutually exclusive.")

                if mat3x3 is not None:
                    self.__attrs_init__(
                        transform=TranslationAndMat3x3(translation=translation, mat3x3=mat3x3, from_parent=from_parent)
                    )
                else:
                    self.__attrs_init__(
                        transform=TranslationRotationScale3D(
                            translation=translation, rotation=rotation, scale=scale, from_parent=from_parent
                        )
                    )
            return

        self.__attrs_clear__()
