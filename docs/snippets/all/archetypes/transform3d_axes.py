"""Log different transforms with visualized coordinates axes."""

import rerun as rr

rr.init("rerun_example_transform3d_axes", spawn=True)

rr.set_time_sequence("step", 0)

# Set the axis lengths for all the transforms
rr.log("base", rr.Transform3D(axis_length=1))
rr.log("base/rotated", rr.Transform3D(axis_length=0.5))
rr.log("base/rotated/translated", rr.Transform3D(axis_length=0.5))

# Now sweep out a rotation relative to the base
for deg in range(360):
    rr.set_time_sequence("step", deg)
    rr.log(
        "base/rotated",
        rr.Transform3D(
            rotation=rr.RotationAxisAngle(
                axis=[1.0, 1.0, 1.0],
                degrees=deg,
            )
        ),
    )
    rr.log(
        "base/rotated/translated",
        rr.Transform3D(
            translation=[2.0, 0, 0],
        ),
    )
