"""Log some random points with color and radii."""
import rerun as rr
from numpy.random import default_rng

rr.init("points", spawn=True)
rng = default_rng(12345)

positions = rng.uniform(-3, 3, size=[10, 2])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr.log_points("random", positions=positions, colors=colors, radii=radii)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 8, 6], rect_format=rr.RectFormat.XCYCWH)
