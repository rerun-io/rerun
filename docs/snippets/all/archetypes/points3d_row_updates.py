"""
Update a point cloud over time.

See also the `points3d_column_updates` example, which achieves the same thing in a single operation.
"""

import numpy as np
import rerun as rr

rr.init("rerun_example_points3d_row_updates", spawn=True)

# Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
times = np.arange(10, 15, 1.0)
# fmt: off
positions = [
    [[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
    [[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
    [[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
    [[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
    [[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
]
# fmt: on

# At each timestep, all points in the cloud share the same but changing color and radius.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
radii = [0.05, 0.01, 0.2, 0.1, 0.3]

for i in range(5):
    rr.set_time("time", duration=10 + i)
    rr.log("points", rr.Points3D(positions[i], colors=colors[i], radii=radii[i]))
