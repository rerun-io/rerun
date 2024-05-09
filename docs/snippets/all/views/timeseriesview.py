"""Use a blueprint to customize a TimeSeriesView."""

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
