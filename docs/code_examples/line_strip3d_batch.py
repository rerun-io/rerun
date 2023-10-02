"""Log a batch of 3d line strips."""
import rerun as rr

rr.init("rerun_example_line_strip3d", spawn=True)

rr.log(
    "strips",
    rr.LineStrips3D(
        [
            [
                [0, 0, 2],
                [1, 0, 2],
                [1, 1, 2],
                [0, 1, 2],
            ],
            [
                [0, 0, 0],
                [0, 0, 1],
                [1, 0, 0],
                [1, 0, 1],
                [1, 1, 0],
                [1, 1, 1],
                [0, 1, 0],
                [0, 1, 1],
            ],
        ],
        colors=[[255, 0, 0], [0, 255, 0]],
        radii=[0.025, 0.005],
        labels=["one strip here", "and one strip there"],
    ),
)
