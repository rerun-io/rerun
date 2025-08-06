from __future__ import annotations

from typing import Any

from .. import components, datatypes
from ..error_utils import catch_and_log_exceptions


class SeriesPointsExt:
    """Extension for [SeriesPoints][rerun.archetypes.SeriesPoints]."""

    def __init__(
        self: Any,
        *,
        colors: datatypes.Rgba32ArrayLike | None = None,
        markers: components.MarkerShapeArrayLike | None = None,
        names: datatypes.Utf8ArrayLike | None = None,
        visible_series: datatypes.BoolArrayLike | None = None,
        marker_sizes: datatypes.Float32ArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the SeriesPoints archetype.

        Parameters
        ----------
        colors:
            Color for the corresponding series.

            May change over time, but can cause discontinuities in the line.
        markers:
            What shape to use to represent the point

            May change over time.
        names:
            Display name of the series.

            Used in the legend. Expected to be unchanging over time.
        visible_series:
            Which lines are visible.

            If not set, all line series on this entity are visible.
            Unlike with the regular visibility property of the entire entity, any series that is hidden
            via this property will still be visible in the legend.

            May change over time.
        marker_sizes:
            Sizes of the markers.

            May change over time.

            If no other components are set, a default `MarkerShape.Circle` will be logged.

        """

        # You can define your own __init__ function as a member of SeriesPointsExt in series_points_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            if all(arg is None for arg in [colors, markers, names, visible_series, marker_sizes]):
                # TODO(#10512): Back when we had indcators, we did'nt need to specify any additional components
                # when logging a `SeriesPoints`. Now that we don't have indicators anymore, we need to have at
                # least one component set in `SeriesPoints`, otherwise nothing would get logged and visualizers
                # couldn't pick up that archetype. To prevent this from happening, we log a circle by default.
                markers = components.MarkerShape.Circle

            self.__attrs_init__(
                colors=colors, markers=markers, names=names, visible_series=visible_series, marker_sizes=marker_sizes
            )
            return
        self.__attrs_clear__()
