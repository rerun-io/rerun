"""Use a blueprint to customize a Spatial3DView."""

import rerun as rr
import rerun.blueprint as rrb
from numpy.random import default_rng

rr.init("rerun_example_spatial_3d", spawn=True)

# Create some random points.
rng = default_rng(12345)
positions = rng.uniform(-5, 5, size=[50, 3])
colors = rng.uniform(0, 255, size=[50, 3])
radii = rng.uniform(0.1, 0.5, size=[50])

rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))
rr.log("box", rr.Boxes3D(half_sizes=[5, 5, 5], colors=0))

# Create a Spatial3D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial3DView(
        origin="/",
        name="3D Scene",
        # Set the background color to light blue.
        background=[100, 149, 237],
        # Configure the eye controls.
        eye_controls=rrb.EyeControls3D(
            position=(0.0, 0.0, 2.0),
            look_target=(0.0, 2.0, 0.0),
            eye_up=(-1.0, 0.0, 0.0),
            spin_speed=0.2,
            kind=rrb.Eye3DKind.FirstPerson,
            speed=20.0,
        ),
        # Configure the line grid.
        line_grid=rrb.LineGrid3D(
            visible=True,  # The grid is enabled by default, but you can hide it with this property.
            spacing=0.1,  # Makes the grid more fine-grained.
            # By default, the plane is inferred from view coordinates setup, but you can set arbitrary planes.
            plane=rr.components.Plane3D.XY.with_distance(-5.0),
            stroke_width=2.0,  # Makes the grid lines twice as thick as usual.
            color=[255, 255, 255, 128],  # Colors the grid a half-transparent white.
        ),
        spatial_information=rrb.SpatialInformation(
            target_frame="tf#/",
            show_axes=True,
            show_bounding_box=True,
        ),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
