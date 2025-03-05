from __future__ import annotations

from typing import Any

from rerun.datatypes.bool import BoolLike

from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions


class PlotLegendExt:
    """Extension for [PlotLegend][rerun.blueprint.archetypes.PlotLegend]."""

    def __init__(
        self: Any,
        corner: blueprint_components.Corner2DLike | None = None,
        *,
        visible: BoolLike | None = None,
    ) -> None:
        """
        Create a new instance of the PlotLegend archetype.

        Parameters
        ----------
        corner:
            To what corner the legend is aligned.

            Defaults to the right bottom corner.
        visible:
            Whether the legend is shown at all.

            True by default.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(corner=corner, visible=visible)
            return
        self.__attrs_clear__()
