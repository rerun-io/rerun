"""Log different transforms."""

import rerun as rr

rr.init("rerun_example_transform3d_axes", spawn=True)

# Make the base axes longer
# Log all axes markers as static first
rr.log("base", rr.Axes3D(length=1), static=True)
rr.log("base/rotated", rr.Axes3D(length=0.5), static=True)
rr.log("base/rotated/translated", rr.Axes3D(length=0.5), static=True)

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
