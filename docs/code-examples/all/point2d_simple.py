"""Log some very simple points."""
import rerun as rr

rr.init("rerun_example_points2d", spawn=True)

rr.log("points", rr.Points2D([[0, 0], [1, 1]]))

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(half_sizes=[2, 1.5]))
