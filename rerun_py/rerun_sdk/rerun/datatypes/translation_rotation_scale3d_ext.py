from __future__ import annotations

from typing import TYPE_CHECKING, Any, Iterable, cast

if TYPE_CHECKING:
    from .._log import ComponentBatchLike
    from . import Rotation3DLike, Scale3DLike, Vec3D, Vec3DLike


class TranslationRotationScale3DExt:
    # TODO(#2641): this is needed until we support default value for params
    def __init__(
        self: Any,
        translation: Vec3DLike | None = None,
        rotation: Rotation3DLike | None = None,
        scale: Scale3DLike | None = None,
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

    # Implement the AsComponents protocol
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..archetypes import Transform3D
        from ..datatypes import TranslationRotationScale3D

        return Transform3D(cast(TranslationRotationScale3D, self)).as_component_batches()

    def num_instances(self) -> int:
        # Always a mono-component
        return 1
