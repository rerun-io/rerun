"""Log a simple line strip."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_line_strip2d", spawn=True)

rr2.log(
    "strip",
    rr2.LineStrips2D([[[0, 0], [2, 1], [4, -1], [6, 0]]]),
)

# Log an extra rect to set the view bounds
rr2.log("bounds", rr2.Boxes2D([4, 3], centers=[3, 0]))
