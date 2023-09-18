"""Log a simple 2D Box."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_box2d", spawn=True)

# TODO(#3268): Use an extension method of rr2.Boxes2D to log an XYWH rect
rr.log_rect("simple", [-1, -1, 2, 2], rect_format=rr.RectFormat.XYWH)

# Log an extra rect to set the view bounds
rr2.log("bounds", rr2.Boxes2D(half_sizes=[2.0, 1.5]))
