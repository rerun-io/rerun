"""Log a batch of 3D arrows."""

from math import tau

import numpy as np

import rerun as rr

rr.init("rerun_example_arrow3d", spawn=True)

lengths = np.log2(np.arange(0, 100) + 1)
angles = tau * np.arange(100, dtype=np.float32) * 0.01
origins = np.zeros((100, 3))
vectors = np.column_stack([
    np.sin(angles) * lengths,
    np.zeros(100),
    np.cos(angles) * lengths,
])
greens = [round(c * 255.0) for c in angles / tau]
colors = [[255 - g, g, 128, 128] for g in greens]

rr.log("arrows", rr.Arrows3D(origins=origins, vectors=vectors, colors=colors))
