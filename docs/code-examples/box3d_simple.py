"""Log a single oriented bounding box."""
import rerun as rr

rr.init("rerun_example_box3d", spawn=True)

rr.log_obb("simple", half_size=[2.0, 2.0, 1.0])
