from __future__ import annotations

from typing import Any

from ..datatypes import AABB2D, Vec2DLike


class AABB2DExt:
    """Extension for [AABB2D][rerun.components.AABB2D]."""

    def __init__(
        self: Any,
        *,
        min: Vec2DLike,
        max: Vec2DLike,
    ):
        """
        Create a new instance of the AABB2D component.

        Parameters
        ----------
        min:
            The minimum point of the visible parts of a 2D space view, in the coordinate space of the scene.
            Usually the left-top corner.
        max:
            The maximum point of the visible parts of a 2D space view, in the coordinate space of the scene.
            Usually the right-bottom corner.

        """

        self.__attrs_init__(AABB2D(min=min, max=max))
