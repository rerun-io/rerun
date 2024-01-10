"""Log some very simple points."""
import rerun as rr

rr.init("rerun_example_points3d_simple", spawn=True)

rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
