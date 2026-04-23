# Use a blueprint to show a StatusView.

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_status", spawn=True)

rr.set_time("step", sequence=0)
rr.log("door", rr.Status(status="open"))

rr.set_time("step", sequence=1)
rr.log("door", rr.Status(status="closed"))

rr.set_time("step", sequence=2)
rr.log("door", rr.Status(status="open"))

# Create a status view to display the status transitions.
blueprint = rrb.Blueprint(
    rrb.StatusView(
        origin="/",
        name="Status Transitions",
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
