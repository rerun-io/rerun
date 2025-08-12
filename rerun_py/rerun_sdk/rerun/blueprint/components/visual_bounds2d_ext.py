from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ...datatypes import Range1DLike


class VisualBounds2DExt:
    """Extension for [VisualBounds2D][rerun.blueprint.components.VisualBounds2D]."""

    def __init__(
        self: Any,
        *,
        x_range: Range1DLike,
        y_range: Range1DLike,
    ) -> None:
        """
        Create a new instance of the VisualBounds2D component.

        Parameters
        ----------
        x_range:
            The minimum visible range of the X-axis (usually left and right bounds).
        y_range:
            The minimum visible range of the Y-axis (usually left and right bounds).

        """

        self.__attrs_init__(x_range=x_range, y_range=y_range)
