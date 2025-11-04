"""Log some random points with color and radii."""

from numpy.random import default_rng

import rerun as rr

rr.init("rerun_example_points3d_random", spawn=True)
rng = default_rng(12345)

positions = rng.uniform(-5, 5, size=[10, 3])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr.log("random", rr.Points3D(positions, colors=colors, radii=radii))
