"""Log some very simple points."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_points2d", spawn=True)

rr.log("points", rr.Points2D([[0, 0], [1, 1]]))

# Set view bounds:
rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1, 2], y_range=[-1, 2])))
