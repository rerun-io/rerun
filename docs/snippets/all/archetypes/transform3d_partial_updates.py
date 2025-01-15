"""Log different transforms with visualized coordinates axes."""

import math

import rerun as rr


def truncated_radians(deg: float) -> float:
    return float(int(math.radians(deg) * 1000.0)) / 1000.0


rr.init("rerun_example_transform3d_partial_updates", spawn=True)

rr.log(
    "box",
    rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
    rr.Transform3D(axis_length=10),
)

for deg in range(46):
    rad = truncated_radians(deg * 4)
    # TODO(#8582): update_fields
    rr.log(
        "box",
        rr.Transform3D(
            # TODO(cmc): we should have access to all the fields of the extended constructor too.
            rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
        ),
    )

for t in range(51):
    # TODO(#8582): update_fields
    rr.log(
        "box",
        rr.Transform3D(translation=[0, 0, t / 10.0]),
    )

for deg in range(46):
    rad = truncated_radians((deg + 45) * 4)
    # TODO(#8582): update_fields
    rr.log(
        "box",
        rr.Transform3D(
            # TODO(cmc): we should have access to all the fields of the extended constructor too.
            rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
        ),
    )

# TODO(#8582): update_fields(clear=True)
rr.log(
    "box",
    rr.Transform3D(axis_length=15),
)
