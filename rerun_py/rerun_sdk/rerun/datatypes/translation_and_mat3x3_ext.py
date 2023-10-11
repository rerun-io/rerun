from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from . import Mat3x3Like, Vec3DLike


class TranslationAndMat3x3Ext:
    """Extension for [TranslationAndMat3x3][rerun.datatypes.TranslationAndMat3x3]."""

    # TODO(#2641): this is needed until we support default value for from_parent
    def __init__(
        self: Any,
        translation: Vec3DLike | None = None,
        mat3x3: Mat3x3Like | None = None,
        *,
        from_parent: bool = False,
    ) -> None:
        """
        Create a new instance of the TranslationAndMat3x3 datatype.

        Parameters
        ----------
        translation:
             3D translation, applied after the matrix.
        mat3x3:
             3x3 matrix for scale, rotation & shear.
        from_parent:
             If true, the transform maps from the parent space to the space where the transform was logged.
             Otherwise, the transform maps from the space to its parent.
        """

        self.__attrs_init__(  # pyright: ignore[reportGeneralTypeIssues]
            translation=translation, mat3x3=mat3x3, from_parent=from_parent
        )
