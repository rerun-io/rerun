"""Log a batch of cylinders."""

import rerun as rr

rr.init("rerun_example_cylinders3d_batch", spawn=True)

rr.log(
    "cylinders",
    rr.Cylinders3D(
        lengths=[0.0, 2.0, 4.0, 6.0, 8.0],
        radii=[1.0, 0.5, 0.5, 0.5, 1.0],
        colors=[
            (255, 0, 0),
            (188, 188, 0),
            (0, 255, 0),
            (0, 188, 188),
            (0, 0, 255),
        ],
        centers=[
            (0.0, 0.0, 0.0),
            (2.0, 0.0, 0.0),
            (4.0, 0.0, 0.0),
            (6.0, 0.0, 0.0),
            (8.0, 0.0, 0.0),
        ],
        rotation_axis_angles=[
            rr.RotationAxisAngle(
                [1.0, 0.0, 0.0],
                rr.Angle(deg=float(i) * -22.5),
            )
            for i in range(5)
        ],
    ),
)
