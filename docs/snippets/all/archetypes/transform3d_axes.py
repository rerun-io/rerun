"""Log different transforms with visualized coordinates axes."""

import rerun as rr

rr.init("rerun_example_transform3d_axes", spawn=True)

rr.set_time("step", sequence=0)

# Set the axis lengths for all the transforms
rr.log("base", rr.Transform3D(), rr.TransformAxes3D(1.0))

# Now sweep out a rotation relative to the base
for deg in range(360):
    rr.set_time("step", sequence=deg)
    rr.log(
        "base/rotated",
        rr.Transform3D.from_fields(
            rotation_axis_angle=rr.RotationAxisAngle(
                axis=[1.0, 1.0, 1.0],
                degrees=deg,
            ),
        ),
        rr.TransformAxes3D(0.5),
    )
    rr.log(
        "base/rotated/translated",
        rr.Transform3D.from_fields(
            translation=[2.0, 0, 0],
        ),
        rr.TransformAxes3D(0.5),
    )
