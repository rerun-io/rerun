"""Log different transforms with visualized coordinates axes."""

import math

import rerun as rr


def truncated_radians(deg: float) -> float:
    return float(int(math.radians(deg) * 1000.0)) / 1000.0


rr.init("rerun_example_transform3d_partial_updates", spawn=True)

# Set up a 3D box.
rr.log(
    "box",
    rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
    rr.Transform3D(clear=False, axis_length=10),
)

# Update only the rotation of the box.
for deg in range(46):
    rad = truncated_radians(deg * 4)
    rr.log(
        "box",
        rr.Transform3D.update_fields(
            # TODO(cmc): we should have access to all the fields of the extended constructor too.
            rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
        ),
    )

# Update only the position of the box.
for t in range(51):
    rr.log(
        "box",
        rr.Transform3D.update_fields(translation=[0, 0, t / 10.0]),
    )

# Update only the rotation of the box.
for deg in range(46):
    rad = truncated_radians((deg + 45) * 4)
    rr.log(
        "box",
        rr.Transform3D.update_fields(
            # TODO(cmc): we should have access to all the fields of the extended constructor too.
            rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
        ),
    )

# Clear all of the box's attributes, and reset its axis length.
rr.log(
    "box",
    rr.Transform3D.update_fields(clear=True, axis_length=15),
)
