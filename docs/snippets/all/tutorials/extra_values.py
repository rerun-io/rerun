"""Log extra values with a `Points2D`."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_extra_values", spawn=True)

rr.log(
    "extra_values",
    rr.Points2D([[-1, -1], [-1, 1], [1, -1], [1, 1]]),
    rr.AnyValues(
        confidence=[0.3, 0.4, 0.5, 0.6],
    ),
)

# Set view bounds:
rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1.5, 1.5], y_range=[-1.5, 1.5])))
