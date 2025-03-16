"""
Update a set of vectors over time, in a single operation.

This is semantically equivalent to the `arrows3d_row_updates` example, albeit much faster.
"""

import numpy as np
import rerun as rr

rr.init("rerun_example_arrows3d_column_updates", spawn=True)

# Prepare a fixed sequence of arrows over 5 timesteps.
# Origins stay constant, vectors change magnitude and direction, and each timestep has a unique color.
times = np.arange(10, 15, 1.0)

# At each time step, all arrows maintain their origin.
origins = [np.linspace((-1, -1, 0), (1, 1, 0), 5)] * 5
vectors = [np.linspace((-1, -1, 0), (1, 1, i), 5) for i in range(5)]


# At each timestep, all arrows share the same but changing color.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]

rr.send_columns(
    "arrows",
    times=[rr.TimeColumn("time", duration=times)],
    columns=[*rr.Arrows3D.columns(origins=origins, vectors=vectors, colors=colors)],
)
