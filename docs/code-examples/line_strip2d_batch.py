"""Log a batch of 2d line strips."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_line_strip2d", spawn=True)

rr2.log(
    "strips",
    rr2.LineStrips2D(
        [
            [[0, 0], [2, 1], [4, -1], [6, 0]],
            [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
        ],
        colors=[[255, 0, 0], [0, 255, 0]],
        radii=[0.025, 0.005],
        labels=["one strip here", "and one strip there"],
    ),
)

# Log an extra rect to set the view bounds
rr2.log("bounds", rr2.Boxes2D([4.0, 4.5], centers=[3, 1.5]))
