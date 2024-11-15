"""Log random points and the corresponding covariance ellipsoid."""

import numpy as np
import rerun as rr

rr.init("rerun_example_ellipsoid_simple", spawn=True)

center = np.array([0, 0, 0])
sigmas = np.array([5, 3, 1])
points = np.random.randn(50_000, 3) * sigmas.reshape(1, -1)

rr.log("points", rr.Points3D(points, radii=0.02, colors=[188, 77, 185]))
rr.log(
    "ellipsoid",
    rr.Ellipsoids3D(
        centers=[center, center],
        half_sizes=[sigmas, 3 * sigmas],
        colors=[[255, 255, 0], [64, 64, 0]],
    ),
)
