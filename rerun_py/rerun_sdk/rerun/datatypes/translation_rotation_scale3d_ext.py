from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from . import Rotation3DLike, Scale3DLike, Vec3D, Vec3DLike


class TranslationRotationScale3DExt:
    """Extension for [TranslationRotationScale3D][rerun.datatypes.TranslationRotationScale3D]."""

    # TODO(#2641): this is needed until we support default value for params
    def __init__(
        self: Any,
        translation: Vec3DLike | None = None,
        rotation: Rotation3DLike | None = None,
        scale: Scale3DLike | None = None,
        *,
        from_parent: bool = False,
    ) -> None:
        self.__attrs_init__(  # pyright: ignore[reportGeneralTypeIssues]
            translation=translation, rotation=rotation, scale=scale, from_parent=from_parent
        )

    @staticmethod
    def translation__field_converter_override(data: Vec3DLike | None) -> Vec3D | None:
        if data is None:
            return None
        else:
            from . import Vec3D

            return Vec3D(data)
