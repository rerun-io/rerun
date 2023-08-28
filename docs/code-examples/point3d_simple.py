"""Log some very simple points."""
import rerun as rr

rr.init("rerun_example_points3d", spawn=True)

rr.log_points("simple", positions=[[0, 0, 0], [1, 1, 1]])
