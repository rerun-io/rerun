"""Use a blueprint to customize a DataframeView."""

import math

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_dataframe", spawn=True)

# Log some data.
for t in range(0, int(math.pi * 4 * 100.0)):
    rr.set_time_seconds("t", t)
    rr.log("trig/sin", rr.Scalar(math.sin(float(t) / 100.0)))
    rr.log("trig/cos", rr.Scalar(math.cos(float(t) / 100.0)))

    # some sparse data
    if t % 5 == 0:
        rr.log("trig/tan_sparse", rr.Scalar(math.tan(float(t) / 100.0)))

# Create a Dataframe View
blueprint = rrb.Blueprint(
    rrb.DataframeView(
        origin="/trig",
        query=rrb.archetypes.DataframeQuery(
            timeline="t",
            filter_by_range=(rr.TimeInt(seconds=0), rr.TimeInt(seconds=20)),
            filter_is_not_null="/trig/tan_sparse:Scalar",
            select=["t", "log_tick", "/trig/sin:Scalar", "/trig/cos:Scalar", "/trig/tan_sparse:Scalar"],
        ),
    ),
)

rr.send_blueprint(blueprint)
