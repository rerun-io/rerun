"""Use a blueprint to customize a Spatial2DView."""

import rerun as rr
import rerun.blueprint as rrb
from numpy.random import default_rng

rr.init("rerun_example_spatial_2d", spawn=True)

# Create some random points.
rng = default_rng(12345)
positions = rng.uniform(-10, 10, size=[50, 2])
colors = rng.uniform(0, 255, size=[50, 3])
radii = rng.uniform(0.25, 0.5, size=[50])

rr.log("points", rr.Points2D(positions, colors=colors, radii=radii))

# Create a Spatial2D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial2DView(
        origin="/points",
        # Set the background color to light blue.
        background=[100, 149, 237],
        # Note that this range is smaller than the range of the points,
        # so some points will not be visible.
        visual_bounds=rrb.VisualBounds(x_range=[-5, 5], y_range=[-5, 5]),
    )
)

rr.send_blueprint(blueprint)
