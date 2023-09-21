"""Change the view coordinates for the scene."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_view_coordinate", spawn=True)

rr2.log("/", rr2.ViewCoordinates.ULB)
rr2.log(
    "xyz",
    rr2.Arrows3D(
        vectors=[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)
