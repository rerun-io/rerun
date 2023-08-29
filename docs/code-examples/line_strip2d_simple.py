"""Log a simple line strip."""
import rerun as rr

rr.init("rerun_example_line_strip2d", spawn=True)

rr.log_line_strip(
    "simple",
    [[0, 0], [2, 1], [4, -1], [6, 0]],
)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [3, 0, 8, 6], rect_format=rr.RectFormat.XCYCWH)
