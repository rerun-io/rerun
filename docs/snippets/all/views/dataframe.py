"""Use a blueprint to customize a DataframeView."""

import math

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_dataframe", spawn=True)

# Log some data.
rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)"), static=True)
rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)"), static=True)
for t in range(0, int(math.pi * 4 * 100.0)):
    rr.set_time_seconds("t", t)
    rr.log("trig/sin", rr.Scalar(math.sin(float(t) / 100.0)))
    rr.log("trig/cos", rr.Scalar(math.cos(float(t) / 100.0)))

# Create a Dataframe View
blueprint = rrb.Blueprint(
    rrb.DataframeView(
        origin="/trig",
        # TODO(#6896): add an interesting query here
    ),
)

rr.send_blueprint(blueprint)
