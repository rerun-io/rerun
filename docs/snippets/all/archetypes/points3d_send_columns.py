"""Use the `send_columns` API to send several point clouds over time in a single call."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_points3d_send_columns", spawn=True)

# Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
times = np.arange(10, 15, 1.0)
# fmt: off
positions = [
    [1.0, 0.0, 1.0], [0.5, 0.5, 2.0],
    [1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0],
    [2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5],
    [-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5],
    [1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0],
]
# fmt: on
positions_arr = np.concatenate(positions)

# At each timestep, all points in the cloud share the same but changing color and radius.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
radii = [0.05, 0.01, 0.2, 0.1, 0.3]

# TODO(#8752): use tagged columnar APIs
rr.send_columns(
    "points",
    times=[rr.TimeSecondsColumn("time", times)],
    components=[
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions_arr).partition([2, 4, 4, 3, 4]),
        rr.components.ColorBatch(colors),
        rr.components.RadiusBatch(radii),
    ],
)
