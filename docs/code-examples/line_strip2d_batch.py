"""Log a batch of 2d line strips."""
import rerun as rr

rr.init("rerun_example_line_strip2d", spawn=True)

rr.log_line_strips_2d(
    "batch",
    [
        [[0, 0], [2, 1], [4, -1], [6, 0]],
        [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
    ],
    colors=[[255, 0, 0], [0, 255, 0]],
    stroke_widths=[0.05, 0.01],
)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [3, 1.5, 8, 9], rect_format=rr.RectFormat.XCYCWH)
