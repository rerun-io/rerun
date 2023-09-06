"""Log some very simple points."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_points2d", spawn=True)

rr2.log("points", rr2.Points2D([[0, 0], [1, 1]]))

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 4, 3], rect_format=rr.RectFormat.XCYCWH)
