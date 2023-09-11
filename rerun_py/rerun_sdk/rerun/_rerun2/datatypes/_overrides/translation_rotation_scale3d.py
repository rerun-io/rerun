from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import Rotation3DLike, Scale3DLike, TranslationRotationScale3D, Vec3DLike


# TODO(#2641): this is needed until we support default value for params
def override_translation_rotation_scale3d__init_override(
    self: TranslationRotationScale3D,
    translation: Vec3DLike | None = None,
    rotation: Rotation3DLike | None = None,
    scale: Scale3DLike | None = None,
    from_parent: bool = False,
) -> None:
    self.__attrs_init__(  # pyright: ignore[reportGeneralTypeIssues]
        translation=translation, rotation=rotation, scale=scale, from_parent=from_parent
    )
