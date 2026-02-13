"""Override a component."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_component_override", spawn=True)

# Data logged to the data store.
rr.log("boxes/1", rr.Boxes2D(centers=[0, 0], sizes=[1, 1], colors=[255, 0, 0]))
rr.log("boxes/2", rr.Boxes2D(centers=[2, 0], sizes=[1, 1], colors=[255, 0, 0]))

rr.send_blueprint(
    rrb.Spatial2DView(
        # Override the values from the data store for the first box.
        overrides={
            "boxes/1": rr.Boxes2D(colors=[0, 255, 0]),
        },
    ),
)
