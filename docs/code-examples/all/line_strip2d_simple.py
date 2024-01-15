"""Log a simple line strip."""
import rerun as rr

rr.init("rerun_example_line_strip2d", spawn=True)

rr.log(
    "strip",
    rr.LineStrips2D([[[0, 0], [2, 1], [4, -1], [6, 0]]]),
)

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(centers=[3, 0], half_sizes=[4, 3]))
