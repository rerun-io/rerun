"""Log a single 3D Box."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_box3d_simple", spawn=True)

rr2.log("simple", rr2.Boxes3D(half_sizes=[2.0, 2.0, 1.0]))
