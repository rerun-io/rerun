"""Log a batch of oriented bounding boxes."""
import rerun as rr
from rerun.datatypes import Angle, Quaternion, Rotation3D, RotationAxisAngle

rr.init("rerun_example_box3d_batch", spawn=True)

rr.log(
    "batch",
    rr.Boxes3D(
        centers=[[2, 0, 0], [-2, 0, 0], [0, 0, 2]],
        half_sizes=[[2.0, 2.0, 1.0], [1.0, 1.0, 0.5], [2.0, 0.5, 1.0]],
        rotations=[
            Rotation3D.identity(),
            Quaternion(xyzw=[0.0, 0.0, 0.382683, 0.923880]),  # 45 degrees around Z
            RotationAxisAngle(axis=[0, 1, 0], angle=Angle(deg=30)),
        ],
        radii=0.025,
        colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
        labels=["red", "green", "blue"],
    ),
)
