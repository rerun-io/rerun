"""Use a blueprint to customize a Spatial2DView."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_spatial_2d", spawn=True)

# Create a spiral of points:
theta = np.linspace(0, 10 * np.pi, 300)
radius = np.linspace(0, 10, 300)
positions = np.column_stack((np.cos(theta) * radius, np.sin(theta) * radius))
colors = np.random.randint(0, 255, size=(len(theta), 3))

rr.log("points", rr.Points2D(positions, colors=colors, radii=0.1))
rr.log("box", rr.Boxes2D(half_sizes=[3, 3], colors=0))

# Create a Spatial2D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial2DView(
        origin="/",
        name="2D Scene",
        # Set the background color to light blue.
        background=[100, 149, 237],
        # Note that this range is smaller than the range of the points,
        # so some points will not be visible.
        visual_bounds=rrb.VisualBounds(x_range=[-5, 5], y_range=[-5, 5]),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
