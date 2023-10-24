"""Log a couple 2D line segments using 2D line strips."""
import numpy as np
import rerun as rr

rr.init("rerun_example_line_segments2d", spawn=True)

rr.log(
    "segments",
    rr.LineStrips2D(np.array([[[0, 0], [2, 1]], [[4, -1], [6, 0]]])),
)

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(centers=[3, 0], half_sizes=[4, 3]))
