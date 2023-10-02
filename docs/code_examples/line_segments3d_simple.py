#!/usr/bin/env python3
"""Log a simple set of line segments."""
import numpy as np
import rerun as rr

rr.init("rerun_example_line_segments3d", spawn=True)

rr.log(
    "segments",
    rr.LineStrips3D(
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
