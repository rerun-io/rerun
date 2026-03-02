"""Use the math/plot convention for 2D (Y pointing up)."""

import rerun as rr

rr.init("rerun_example_view_coordinates2d", spawn=True)

rr.log("world", rr.ViewCoordinates2D.RU, static=True)  # Set Y-Up

rr.log(
    "world/points",
    rr.Points2D([(0, 0), (1, 1), (3, 2)]),
)
