from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ...error_utils import catch_and_log_exceptions
from .. import components as blueprint_components

if TYPE_CHECKING:
    from ... import datatypes


class VisualBounds2DExt:
    """Extension for [VisualBounds2D][rerun.blueprint.archetypes.VisualBounds2D]."""

    def __init__(
        self: Any,
        *,
        x_range: datatypes.Range1DLike | None = None,
        y_range: datatypes.Range1DLike | None = None,
    ) -> None:
        """
        Create a new instance of the VisualBounds2D archetype.

        Parameters
        ----------
        x_range:
            The minimum visible range of the X-axis (usually left and right bounds).
        y_range:
            The minimum visible range of the Y-axis (usually left and right bounds).

        """

        if x_range is not None and y_range is not None:
            range = blueprint_components.VisualBounds2D(x_range=x_range, y_range=y_range)
        elif x_range is not None or y_range is not None:
            raise ValueError("Both x_range and y_range must be specified.")
        else:
            range = None

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                range=range,
            )
            return
        self.__attrs_clear__()
