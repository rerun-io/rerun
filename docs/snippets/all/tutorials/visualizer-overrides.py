"""Log a scalar over time and override the visualizer."""

from math import cos, sin, tau

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_series_line_overrides", spawn=True)

# Log the data on a timeline called "step".
for t in range(int(tau * 2 * 10.0)):
    rr.set_time("step", sequence=t)

    rr.log("trig/sin", rr.Scalars(sin(float(t) / 10.0)))
    rr.log("trig/cos", rr.Scalars(cos(float(t) / 10.0)))

# Use the SeriesPoints visualizer for the sin series.
rr.send_blueprint(
    rrb.TimeSeriesView(
        overrides={
            "trig/sin": [
                rrb.visualizers.SeriesLines(),
                rrb.visualizers.SeriesPoints(),
            ],
        },
    ),
)
