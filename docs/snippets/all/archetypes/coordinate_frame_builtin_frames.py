"""Demonstrates using explicit `CoordinateFrame` with implicit transform frames only."""

import rerun as rr

rr.init("rerun_example_transform3d_hierarchy", spawn=True)

rr.set_time("time", sequence=0)
rr.log(
    "red_box",
    rr.Boxes3D(half_sizes=[0.5, 0.5, 0.5], colors=[255, 0, 0]),
    # Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
    rr.Transform3D(translation=[2.0, 0.0, 0.0]),
)
rr.log(
    "blue_box",
    rr.Boxes3D(half_sizes=[0.5, 0.5, 0.5], colors=[0, 0, 255]),
    # Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
    rr.Transform3D(translation=[-2.0, 0.0, 0.0]),
)
rr.log("point", rr.Points3D([0.0, 0.0, 0.0], radii=0.5))

# Change where the point is located by cycling through its coordinate frame.
for t, frame_id in enumerate(["tf#/red_box", "tf#/blue_box"]):
    rr.set_time("time", sequence=t + 1)  # leave it untouched at t==0.
    rr.log("point", rr.CoordinateFrame(frame_id))
