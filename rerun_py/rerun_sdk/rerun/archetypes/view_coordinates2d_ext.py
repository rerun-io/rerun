from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..components import ViewCoordinates2D as Component

if TYPE_CHECKING:
    from ..components import ViewCoordinates2D


class ViewCoordinates2DExt:
    """Extension for [ViewCoordinates2D][rerun.archetypes.ViewCoordinates2D]."""

    RD: ViewCoordinates2D = None  # type: ignore[assignment]
    """X=Right, Y=Down (default, image/screen convention)."""

    RU: ViewCoordinates2D = None  # type: ignore[assignment]
    """X=Right, Y=Up (math/plot convention)."""

    LD: ViewCoordinates2D = None  # type: ignore[assignment]
    """X=Left, Y=Down (horizontally mirrored image)."""

    LU: ViewCoordinates2D = None  # type: ignore[assignment]
    """X=Left, Y=Up (both axes flipped)."""

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.RD = Component.RD
        cls.RU = Component.RU
        cls.LD = Component.LD
        cls.LU = Component.LU
