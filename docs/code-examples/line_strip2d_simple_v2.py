"""Log a simple line strip."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun-example-line_strip2d", spawn=True)

rr2.log(
    "strip",
    rr2.LineStrips2D([[[0, 0], [2, 1], [4, -1], [6, 0]]]),
)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [3, 0, 8, 6], rect_format=rr.RectFormat.XCYCWH)
