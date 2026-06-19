"""Log a batch of 2D ellipses."""

import rerun as rr

rr.init("rerun_example_ellipses2d_batch", spawn=True)

rr.log(
    "batch",
    rr.Ellipses2D(
        centers=[(-2.0, 0.0), (0.0, 0.0), (2.5, 0.0)],
        half_sizes=[(1.5, 0.75), (0.5, 0.5), (0.75, 1.5)],
        line_radii=[0.025, 0.05, 0.025],
        colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
        labels=["wide", "circle", "tall"],
    ),
)
