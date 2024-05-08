# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/views/time_series.fbs".

from __future__ import annotations

from typing import Sequence

__all__ = ["TimeSeriesView"]


from ..._baseclasses import AsComponents
from ...datatypes import EntityPathLike, Utf8Like
from .. import archetypes as blueprint_archetypes
from .. import components as blueprint_components
from ..api import SpaceView, SpaceViewContentsLike


class TimeSeriesView(SpaceView):
    """
    **View**: A time series view.

    Example
    -------
    ### Use a blueprint to customize a TimeSeriesView:
    ```python
    import math

    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_timeseries", spawn=True)

    # Log some trigonometric functions
    rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)"), static=True)
    rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)"), static=True)
    rr.log("trig/cos", rr.SeriesLine(color=[0, 0, 255], name="cos(0.01t) scaled"), static=True)
    for t in range(0, int(math.pi * 4 * 100.0)):
        rr.set_time_sequence("timeline0", t)
        rr.set_time_sequence("timeline1", t)
        rr.log("trig/sin", rr.Scalar(math.sin(float(t) / 100.0)))
        rr.log("trig/cos", rr.Scalar(math.cos(float(t) / 100.0)))
        rr.log("trig/cos_scaled", rr.Scalar(math.cos(float(t) / 100.0) * 2.0))

    # Create a TimeSeries View
    blueprint = rrb.Blueprint(
        rrb.TimeSeriesView(
            origin="/trig",
            # Set a custom Y axis.
            axis_y=rrb.ScalarAxis(range=(-1.0, 1.0), lock_range_during_zoom=True),
            # Configure the legend.
            plot_legend=rrb.PlotLegend(visible=False),
            # Set time different time ranges for different timelines.
            time_ranges=[
                # Sliding window depending on the time cursor for the first timeline.
                rrb.VisibleTimeRange(
                    "timeline0",
                    start=rrb.TimeRangeBoundary.cursor_relative(-100),
                    end=rrb.TimeRangeBoundary.cursor_relative(),
                ),
                # Time range from some point to the end of the timeline for the second timeline.
                rrb.VisibleTimeRange(
                    "timeline1",
                    start=rrb.TimeRangeBoundary.absolute(300),
                    end=rrb.TimeRangeBoundary.infinite(),
                ),
            ],
        )
    )

    rr.send_blueprint(blueprint)
    ```

    """

    def __init__(
        self,
        *,
        origin: EntityPathLike = "/",
        contents: SpaceViewContentsLike = "$origin/**",
        name: Utf8Like | None = None,
        visible: blueprint_components.VisibleLike | None = None,
        axis_y: blueprint_archetypes.ScalarAxis | None = None,
        plot_legend: blueprint_archetypes.PlotLegend | blueprint_components.Corner2D | None = None,
        time_ranges: blueprint_archetypes.VisibleTimeRanges
        | blueprint_components.VisibleTimeRangeLike
        | Sequence[blueprint_components.VisibleTimeRangeLike]
        | None = None,
    ) -> None:
        """
        Construct a blueprint for a new TimeSeriesView view.

        Parameters
        ----------
        origin:
            The `EntityPath` to use as the origin of this view.
            All other entities will be transformed to be displayed relative to this origin.
        contents:
            The contents of the view specified as a query expression.
            This is either a single expression, or a list of multiple expressions.
            See [rerun.blueprint.archetypes.SpaceViewContents][].
        name:
            The display name of the view.
        visible:
            Whether this view is visible.

            Defaults to true if not specified.
        axis_y:
            Configures the vertical axis of the plot.
        plot_legend:
            Configures the legend of the plot.
        time_ranges:
            Configures which range on each timeline is shown by this view (unless specified differently per entity).

        """

        properties: dict[str, AsComponents] = {}
        if axis_y is not None:
            if not isinstance(axis_y, blueprint_archetypes.ScalarAxis):
                axis_y = blueprint_archetypes.ScalarAxis(axis_y)
            properties["ScalarAxis"] = axis_y

        if plot_legend is not None:
            if not isinstance(plot_legend, blueprint_archetypes.PlotLegend):
                plot_legend = blueprint_archetypes.PlotLegend(plot_legend)
            properties["PlotLegend"] = plot_legend

        if time_ranges is not None:
            if not isinstance(time_ranges, blueprint_archetypes.VisibleTimeRanges):
                time_ranges = blueprint_archetypes.VisibleTimeRanges(time_ranges)
            properties["VisibleTimeRanges"] = time_ranges

        super().__init__(
            class_identifier="TimeSeries",
            origin=origin,
            contents=contents,
            name=name,
            visible=visible,
            properties=properties,
        )
