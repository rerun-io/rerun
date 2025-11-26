"""
Update a transform over time.

See also the `transform3d_column_updates` example, which achieves the same thing in a single operation.
"""

import math

import rerun as rr


def truncated_radians(deg: float) -> float:
    return float(int(math.radians(deg) * 1000.0)) / 1000.0


rr.init("rerun_example_transform3d_row_updates", spawn=True)

rr.set_time("tick", sequence=0)
rr.log(
    "box",
    rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
    rr.TransformAxes3D(10.0),
)

for t in range(100):
    rr.set_time("tick", sequence=t + 1)
    rr.log(
        "box",
        rr.Transform3D(
            translation=[0, 0, t / 10.0],
            rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=truncated_radians(t * 4)),
        ),
    )
