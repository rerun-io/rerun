"""Log a batch of 3D arrows."""
from math import tau

import numpy as np
import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

rr.init("arrow", spawn=True)

lengths = np.log2(np.arange(0, 100) + 1)
angles = np.arange(start=0, stop=tau, step=tau * 0.01)
vectors = np.column_stack([np.sin(angles) * lengths, np.zeros(100), np.cos(angles) * lengths])
arrows = [rrd.Arrow3D(origin=[0, 0, 0], vector=v) for v in vectors]
colors = [[1.0 - c, c, 0.5, 0.5] for c in angles / tau]

rr2.log("arrows", rr2.Arrows3D(arrows, colors=colors))
