"""Logs a simple transform hierarchy with named frames."""

import rerun as rr
import numpy as np

rr.init("explicit_frames_example", spawn=True)
rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

# Define entities with explicit coordinate frames
rr.log("sun", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[1, 1, 1], colors=[255, 200, 10]),
       rr.CoordinateFrame("sun_frame"))
rr.log("planet", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.4, 0.4, 0.4], colors=[40, 80, 200]),
       rr.CoordinateFrame("planet_frame"))
rr.log("moon", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.15, 0.15, 0.15], colors=[180, 180, 180]),
       rr.CoordinateFrame("moon_frame"))

# Connect the viewer to the sun's coordinate frame
rr.log("/", rr.CoordinateFrame("sun_frame"))

# Define explicit frame relationships
rr.log("planet_transform", rr.Transform3D(
    translation=[6.0, 0.0, 0.0],
    child_frame="planet_frame",
    parent_frame="sun_frame"
))
rr.log("moon_transform", rr.Transform3D(
    translation=[3.0, 0.0, 0.0],
    child_frame="moon_frame",
    parent_frame="planet_frame"
))
