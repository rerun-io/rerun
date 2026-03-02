from __future__ import annotations

from enum import IntEnum
from typing import TYPE_CHECKING, Any, cast

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from .._baseclasses import DescribedComponentBatch
    from ..components import ViewCoordinates2D as ViewCoordinates2DComponent
    from . import ViewCoordinates2D


class ViewCoordinates2DExt:
    """Extension for [ViewCoordinates2D][rerun.components.ViewCoordinates2D]."""

    class ViewDir2D(IntEnum):
        Up = 1
        Down = 2
        Right = 3
        Left = 4

    @staticmethod
    def coordinates__field_converter_override(data: npt.ArrayLike) -> npt.NDArray[np.uint8]:
        coordinates = np.asarray(data, dtype=np.uint8)
        if coordinates.shape != (2,):
            raise ValueError(f"ViewCoordinates2D must be a 2-element array. Got: {coordinates.shape}")
        return coordinates

    # Implement the AsComponents protocol
    def as_component_batches(self) -> list[DescribedComponentBatch]:
        from ..archetypes import ViewCoordinates2D

        return ViewCoordinates2D(cast("ViewCoordinates2DComponent", self)).as_component_batches()

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
        cls.RD = cls([cls.ViewDir2D.Right, cls.ViewDir2D.Down])
        cls.RU = cls([cls.ViewDir2D.Right, cls.ViewDir2D.Up])
        cls.LD = cls([cls.ViewDir2D.Left, cls.ViewDir2D.Down])
        cls.LU = cls([cls.ViewDir2D.Left, cls.ViewDir2D.Up])
