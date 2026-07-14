"""Shows how to log a GridMap at a specific pose."""

import math
from pathlib import Path

from PIL import Image as PILImage

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_grid_map_pose", spawn=True)

# Log the transform for the map origin.
# Here we use ROS TF-style parent & child frame names.
rr.log(
    "/tf",
    rr.Transform3D(
        translation=[1.0, 2.0, 0.0],
        rotation_axis_angle=rr.components.RotationAxisAngle(
            [0, 0, 1], -math.pi / 3
        ),
        parent_frame="world",
        child_frame="map",
    ),
    static=True,
)

# We use a dummy image for the map in this example.
image = PILImage.open(Path(__file__).parent / "ferris.png").convert("RGBA")

# Log the grid map at the map origin.
rr.log(
    "demo_map",
    rr.CoordinateFrame("map"),
    rr.GridMap(
        data=image.tobytes(),
        format=rr.components.ImageFormat(
            width=image.size[0],
            height=image.size[1],
            color_model="RGBA",
            channel_datatype="U8",
        ),
        opacity=0.5,
        # The size of a pixel in scene units.
        cell_size=0.01,
        # Specify the pose of the lower-left image corner relative to the
        # map frame, in scene units.
        translation=[1.1, -1.6, 0.0],
        rotation_axis_angle=rr.components.RotationAxisAngle(
            [0, 0, 1], math.pi / 4.0
        ),
    ),
)

# Show transform axes with frame names.
rr.send_blueprint(
    rrb.Spatial3DView(
        origin="/",
        overrides={
            "/tf": [rr.TransformAxes3D(axis_length=0.5, show_frame=True)],
        },
    )
)
