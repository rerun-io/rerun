"""Log a simple set of line segments."""
import rerun as rr

rr.init("rerun-example-line_segments3d", spawn=True)

rr.log_line_segments(
    "simple",
    [
        [0, 0, 0],
        [0, 0, 1],
        [1, 0, 0],
        [1, 0, 1],
        [1, 1, 0],
        [1, 1, 1],
        [0, 1, 0],
        [0, 1, 1],
    ],
)
