"""Log extra values with a Points2D."""
import rerun as rr

rr.init("rerun_example_extra_values", spawn=True)

rr.log(
    "extra_values",
    rr.Points2D([[-1, -1], [-1, 1], [1, -1], [1, 1]]),
    rr.AnyValues(
        confidence=[0.3, 0.4, 0.5, 0.6],
    ),
)

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(sizes=[3, 3]))
