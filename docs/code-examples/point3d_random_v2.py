"""Log some random points with color and radii."""
import rerun as rr
import rerun.experimental as rr2
from numpy.random import default_rng

rr.init("rerun-example-points3d_random", spawn=True)
rng = default_rng(12345)

positions = rng.uniform(-5, 5, size=[10, 3])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr2.log("random", rr2.Points3D(positions, colors=colors, radii=radii))
