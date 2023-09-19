"""Log a simple 2D Box."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_box2d", spawn=True)

rr2.log("simple", rr2.Boxes2D(mins=[-1, -1], sizes=[2, 2]))

# Log an extra rect to set the view bounds
rr2.log("bounds", rr2.Boxes2D(sizes=[4.0, 3.0]))
