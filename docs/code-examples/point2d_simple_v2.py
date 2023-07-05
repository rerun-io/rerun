"""Log some very simple points."""
import rerun as rr

rr.init("points", spawn=True)

rr.log_any("simple", rr.Points2D([[0, 0], [1, 1]]))

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 4, 3], rect_format=rr.RectFormat.XCYCWH)
