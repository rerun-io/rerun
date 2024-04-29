from __future__ import annotations

from typing import Any

from ... import components, datatypes
from ...error_utils import catch_and_log_exceptions


class VisualBoundsExt:
    """Extension for [VisualBounds][rerun.blueprint.archetypes.VisualBounds]."""

    def __init__(
        self: Any,
        *,
        min: datatypes.Vec2DLike,
        max: datatypes.Vec2DLike,
    ):
        """
        Create a new instance of the VisualBounds archetype.

        Parameters
        ----------
        min:
            The minimum point of the visible parts of a 2D space view, in the coordinate space of the scene.
            Usually the left-top corner.
        max:
            The maximum point of the visible parts of a 2D space view, in the coordinate space of the scene.
            Usually the right-bottom corner.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(visual_bounds=components.AABB2D(min=min, max=max))
            return
        self.__attrs_clear__()
