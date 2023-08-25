"""Log a simple set of line segments."""
import numpy as np
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun-example-line_segments3d", spawn=True)

rr2.log(
    "segments",
    rr2.LineStrips3D(
        np.array(
            [
                [[0, 0, 0], [0, 0, 1]],
                [[1, 0, 0], [1, 0, 1]],
                [[1, 1, 0], [1, 1, 1]],
                [[0, 1, 0], [0, 1, 1]],
            ],
        )
    ),
)
