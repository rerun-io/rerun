"""Log some random points with color and radii."""
import rerun as rr
from numpy.random import default_rng

rr.init("rerun_example_points2d_random", spawn=True)
rng = default_rng(12345)

positions = rng.uniform(-3, 3, size=[10, 2])
colors = rng.uniform(0, 255, size=[10, 4])
radii = rng.uniform(0, 1, size=[10])

rr.log("random", rr.Points2D(positions, colors=colors, radii=radii))

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(half_sizes=[4, 3]))
