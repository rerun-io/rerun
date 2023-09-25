"""Change the view coordinates for the scene."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_view_coordinates", spawn=True)

rr2.log("world", rr2.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)  # Set an up-axis
rr2.log(
    "world/xyz",
    rr2.Arrows3D(
        vectors=[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)
