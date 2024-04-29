"""Log a simple line strip."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_line_strip2d", spawn=True)

rr.log(
    "strip",
    rr.LineStrips2D([[[0, 0], [2, 1], [4, -1], [6, 0]]]),
)

# Set view bounds:
rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds(min=[-1, -3], max=[7, 3])))
