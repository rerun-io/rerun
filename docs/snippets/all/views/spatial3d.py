"""Use a blueprint to customize a Spatial3DView."""

import rerun as rr
import rerun.blueprint as rrb
from numpy.random import default_rng

rr.init("rerun_example_spatial_3d", spawn=True)

# Create some random points.
rng = default_rng(12345)
positions = rng.uniform(-5, 5, size=[10, 3])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))

# Create a Spatial3D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial3DView(
        origin="/points",
        name="3D Points",
        # Set the background color to light blue.
        background=[100, 149, 237],
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
