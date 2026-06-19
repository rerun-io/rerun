"""Log a simple 2D ellipse."""

import rerun as rr

rr.init("rerun_example_ellipses2d", spawn=True)

rr.log("simple", rr.Ellipses2D(half_sizes=[(2.0, 1.0)], centers=[(0.0, 0.0)]))
