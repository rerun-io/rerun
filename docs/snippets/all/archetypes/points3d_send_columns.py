"""Use the `send_columns` API to send several point clouds over time in a single call."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_send_columns_arrays", spawn=True)

# Prepare a point cloud that evolves over time 5 timesteps, changing the number of points in the process.
times = np.arange(10, 15, 1.0)
positions = [
    [[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
    [[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
    [[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
    [[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
    [[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
]
positions_arr = np.concatenate(positions)

# At each time stamp, all points in the cloud share the same but changing color.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]

rr.send_columns(
    "points",
    times=[rr.TimeSecondsColumn("time", times)],
    components=[
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions_arr).partition([len(row) for row in positions]),
        rr.components.ColorBatch(colors),
    ],
)
