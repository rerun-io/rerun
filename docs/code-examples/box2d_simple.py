"""Log a simple 2D Box."""
import rerun as rr

rr.init("rerun_example_box2d", spawn=True)

rr.log("simple", rr.Boxes2D(mins=[-1, -1], sizes=[2, 2]))

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(sizes=[4.0, 3.0]))
