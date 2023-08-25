"""Log a simple set of line segments."""
import rerun as rr

rr.init("rerun-example-line_segments2d", spawn=True)

rr.log_line_segments(
    "simple",
    [[0, 0], [2, 1], [4, -1], [6, 0]],
)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [3, 0, 8, 6], rect_format=rr.RectFormat.XCYCWH)
