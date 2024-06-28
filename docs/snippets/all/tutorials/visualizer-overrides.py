"""Log a scalar over time and override the visualizer."""

from math import cos, sin, tau

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_series_line_overrides", spawn=True)

# Log the data on a timeline called "step".
for t in range(0, int(tau * 2 * 10.0)):
    rr.set_time_sequence("step", t)

    rr.log("trig/sin", rr.Scalar(sin(float(t) / 10.0)))
    rr.log("trig/cos", rr.Scalar(cos(float(t) / 10.0)))

# Use the SeriesPoint visualizer for the sin series.
rr.send_blueprint(
    rrb.TimeSeriesView(
        overrides={
            "trig/sin": [
                rrb.VisualizerOverrides(rrb.visualizers.SeriesPoint),
            ]
        },
    ),
)
