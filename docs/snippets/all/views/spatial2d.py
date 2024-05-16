"""Use a blueprint to customize a Spatial2DView."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_spatial_2d", spawn=True)

# Create a spiral of points:
n = 150
angle = np.linspace(0, 10 * np.pi, n)
spiral_radius = np.linspace(0.0, 3.0, n) ** 2
positions = np.column_stack((np.cos(angle) * spiral_radius, np.sin(angle) * spiral_radius))
colors = np.dstack((np.linspace(255, 255, n), np.linspace(255, 0, n), np.linspace(0, 255, n)))[0].astype(int)
radii = np.linspace(0.01, 0.7, n)

rr.log("points", rr.Points2D(positions, colors=colors, radii=radii))

# Create a Spatial2D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial2DView(
        origin="/",
        name="2D Scene",
        # Set the background color
        background=[105, 20, 105],
        # Note that this range is smaller than the range of the points,
        # so some points will not be visible.
        visual_bounds=rrb.VisualBounds2D(x_range=[-5, 5], y_range=[-5, 5]),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
