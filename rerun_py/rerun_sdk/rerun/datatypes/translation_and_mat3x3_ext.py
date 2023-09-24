from __future__ import annotations

from typing import TYPE_CHECKING, Any, Iterable, cast

if TYPE_CHECKING:
    from ..log import ComponentBatchLike
    from . import Mat3x3Like, Vec3DLike


class TranslationAndMat3x3Ext:
    # TODO(#2641): this is needed until we support default value for from_parent
    def __init__(
        self: Any,
        translation: Vec3DLike | None = None,
        matrix: Mat3x3Like | None = None,
        from_parent: bool = False,
    ) -> None:
        self.__attrs_init__(  # pyright: ignore[reportGeneralTypeIssues]
            translation=translation, matrix=matrix, from_parent=from_parent
        )

    # Implement the ArchetypeLike
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..archetypes import Transform3D
        from ..datatypes import TranslationAndMat3x3

        return Transform3D(cast(TranslationAndMat3x3, self)).as_component_batches()

    def num_instances(self) -> int:
        # Always a mono-component
        return 1
