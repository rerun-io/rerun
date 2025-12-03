"""Logs a simple transform hierarchy with named frames."""

import rerun as rr

rr.init("rerun_example_transform3d_hierarchy_named_frames", spawn=True)

# Define entities with explicit coordinate frames.
rr.log(
    "sun",
    rr.Ellipsoids3D(half_sizes=[1, 1, 1], colors=[255, 200, 10], fill_mode="solid"),
    rr.CoordinateFrame("sun_frame"),
)
rr.log(
    "planet",
    rr.Ellipsoids3D(half_sizes=[0.4, 0.4, 0.4], colors=[40, 80, 200], fill_mode="solid"),
    rr.CoordinateFrame("planet_frame"),
)
rr.log(
    "moon",
    rr.Ellipsoids3D(half_sizes=[0.15, 0.15, 0.15], colors=[180, 180, 180], fill_mode="solid"),
    rr.CoordinateFrame("moon_frame"),
)

# Define explicit frame relationships.
rr.log(
    "planet_transform",
    rr.Transform3D(translation=[6.0, 0.0, 0.0], child_frame="planet_frame", parent_frame="sun_frame"),
)
rr.log(
    "moon_transform", rr.Transform3D(translation=[3.0, 0.0, 0.0], child_frame="moon_frame", parent_frame="planet_frame")
)

# Connect the viewer to the sun's coordinate frame.
# This is only needed in the absence of blueprints since a default view will typically be created at `/`.
rr.log("/", rr.CoordinateFrame("sun_frame"), static=True)
