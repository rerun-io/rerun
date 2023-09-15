"""Log a batch of oriented bounding boxes."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_box3d_batch", spawn=True)

rr2.log(
    "batch",
    rr2.Boxes3D(
        centers=[[2, 0, 0], [-2, 0, 0], [0, 0, 2]],
        half_sizes=[[2.0, 2.0, 1.0], [1.0, 1.0, 0.5], [2.0, 0.5, 1.0]],
        rotations=[
            rr2.cmp.Rotation3D.identity(),
            rr2.dt.Quaternion(xyzw=[0.0, 0.0, 0.382683, 0.923880]),  # 45 degrees around Z
            rr2.dt.RotationAxisAngle(axis=[0, 1, 0], angle=rr2.dt.Angle(deg=30)),
        ],
        radii=0.025,
        colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
        labels=["red", "green", "blue"],
    ),
)
