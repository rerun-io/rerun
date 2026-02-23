"""Use a blueprint to customize a DataframeView."""

import math

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_dataframe", spawn=True)

# Log some data.
for t in range(int(math.pi * 4 * 100.0)):
    rr.set_time("t", duration=t)
    rr.log("trig/sin", rr.Scalars(math.sin(float(t) / 100.0)))
    rr.log("trig/cos", rr.Scalars(math.cos(float(t) / 100.0)))

    # some sparse data
    if t % 5 == 0:
        rr.log("trig/tan_sparse", rr.Scalars(math.tan(float(t) / 100.0)))

# Create a Dataframe View
blueprint = rrb.Blueprint(
    rrb.DataframeView(
        origin="/trig",
        query=rrb.archetypes.DataframeQuery(
            timeline="t",
            filter_by_range=(rr.TimeInt(seconds=0), rr.TimeInt(seconds=20)),
            filter_is_not_null="/trig/tan_sparse:Scalar",
            select=["t", "log_tick", "/trig/sin:Scalar", "/trig/cos:Scalar", "/trig/tan_sparse:Scalar"],
            entity_order=["/trig/cos", "/trig/sin", "/trig/tan_sparse"],
            auto_scroll=True,
        ),
    ),
)

rr.send_blueprint(blueprint)
