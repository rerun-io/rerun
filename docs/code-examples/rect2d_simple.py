"""Log a simple rectangle."""
import rerun as rr

rr.init("rerun_example_rect2d", spawn=True)

rr.log_rect("simple", [-1, -1, 2, 2], rect_format=rr.RectFormat.XYWH)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 4, 3], rect_format=rr.RectFormat.XCYCWH)
