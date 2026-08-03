"""Set the default orientation for a 3D view."""

import rerun as rr

rr.init("rerun_example_view_coordinates", spawn=True)

rr.log(
    "world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True
)  # Set the 3D view's up direction
rr.log(
    "world/xyz",
    rr.Arrows3D(
        vectors=[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)
