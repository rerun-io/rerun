"""Log a simple 3D box with a regular & instance pose transform."""

import numpy as np
import rerun as rr

rr.init("rerun_example_instance_pose3d_combined", spawn=True)

rr.set_time("frame", sequence=0)

# Log a box and points further down in the hierarchy.
rr.log("world/box", rr.Boxes3D(half_sizes=[[1.0, 1.0, 1.0]]))
rr.log("world/box/points", rr.Points3D(np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-10, 10, 10j)]]]).T))

for i in range(180):
    rr.set_time("frame", sequence=i)

    # Log a regular transform which affects both the box and the points.
    rr.log("world/box", rr.Transform3D(rotation_axis_angle=rr.RotationAxisAngle([0, 0, 1], angle=rr.Angle(deg=i * 2))))

    # Log an instance pose which affects only the box.
    rr.log("world/box", rr.InstancePoses3D(translations=[0, 0, abs(i * 0.1 - 5.0) - 5.0]))
