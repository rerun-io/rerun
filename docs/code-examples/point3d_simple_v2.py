"""Log some very simple points."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun-example-points3d_simple", spawn=True)

rr2.log("simple", rr2.Points3D([[0, 0, 0], [1, 1, 1]]))
