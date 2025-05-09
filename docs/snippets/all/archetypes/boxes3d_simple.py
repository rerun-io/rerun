"""Log a single 3D Box."""

import rerun as rr

rr.init("rerun_example_box3d", spawn=True)

rr.log("simple", rr.Boxes3D(half_sizes=[2.0, 2.0, 1.0]), rr.InstancePoses3D(translations=[1, 2, 3]))
