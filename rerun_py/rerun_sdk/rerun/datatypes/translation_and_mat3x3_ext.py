from __future__ import annotations

from typing import TYPE_CHECKING, Any, Iterable, cast

if TYPE_CHECKING:
    from .._log import ComponentBatchLike
    from . import Mat3x3Like, Vec3DLike


class TranslationAndMat3x3Ext:
    # TODO(#2641): this is needed until we support default value for from_parent
    def __init__(
        self: Any,
        translation: Vec3DLike | None = None,
        matrix: Mat3x3Like | None = None,
        *,
        from_parent: bool = False,
    ) -> None:
        """
        Create a new instance of the TranslationAndMat3x3 datatype.

        Parameters
        ----------
        translation:
             3D translation, applied after the matrix.
        matrix:
             3x3 matrix for scale, rotation & shear.
        from_parent:
             If true, the transform maps from the parent space to the space where the transform was logged.
             Otherwise, the transform maps from the space to its parent.
        """

        self.__attrs_init__(  # pyright: ignore[reportGeneralTypeIssues]
            translation=translation, matrix=matrix, from_parent=from_parent
        )
