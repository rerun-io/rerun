"""Log some random points with color and radii."""
import rerun as rr
import rerun.experimental as rr_exp
from numpy.random import default_rng

rr.init("points", spawn=True)
rng = default_rng(12345)

positions = rng.uniform(-5, 5, size=[10, 3])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr_exp.log_any("random", rr_exp.Points3D(positions, colors=colors, radii=radii))
