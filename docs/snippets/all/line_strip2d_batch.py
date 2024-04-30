"""Log a batch of 2D line strips."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_line_strip2d_batch", spawn=True)

rr.log(
    "strips",
    rr.LineStrips2D(
        [
            [[0, 0], [2, 1], [4, -1], [6, 0]],
            [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
        ],
        colors=[[255, 0, 0], [0, 255, 0]],
        radii=[0.025, 0.005],
        labels=["one strip here", "and one strip there"],
    ),
)

# Set view bounds:
rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds(x_range[-1, 7], y_range=[-3, 6])))
