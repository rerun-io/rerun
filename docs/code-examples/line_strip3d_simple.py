"""Log a simple line strip."""
import rerun as rr

rr.init("linestrip3d", spawn=True)

rr.log_line_strip(
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
