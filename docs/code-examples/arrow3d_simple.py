"""Log a batch of 3D arrows."""
from math import tau

import numpy as np
import rerun as rr

rr.init("rerun_example_arrow3d", spawn=True)

lengths = np.log2(np.arange(0, 100) + 1)
angles = np.arange(start=0, stop=tau, step=tau * 0.01)
origins = np.zeros((100, 3))
vectors = np.column_stack([np.sin(angles) * lengths, np.zeros(100), np.cos(angles) * lengths])
colors = [[1.0 - c, c, 0.5, 0.5] for c in angles / tau]

rr.log("arrows", rr.Arrows3D(origins=origins, vectors=vectors, colors=colors))
