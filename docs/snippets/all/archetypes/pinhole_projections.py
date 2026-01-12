"""Demonstrates pinhole camera projections with Rerun blueprints."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_pinhole_projections", spawn=True)

img_height, img_width = 12, 16

# Create a 3D scene with a camera and an image.
rr.log("world/box", rr.Boxes3D(centers=[0, 0, 0], half_sizes=[1, 1, 1], colors=[255, 0, 0]))
rr.log(
    "world/points",
    rr.Points3D(
        positions=[(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1)],
        colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (255, 0, 255)],
        radii=0.1,
    ),
)
rr.log(
    "camera",
    rr.Transform3D(translation=[0, 3, 0]),
    rr.Pinhole(width=img_width, height=img_height, focal_length=10, camera_xyz=rr.ViewCoordinates.LEFT_HAND_Z_UP),
)
# Create a simple test image.
checkerboard = np.zeros((img_height, img_width, 1), dtype=np.uint8)
checkerboard[(np.arange(img_height)[:, None] + np.arange(img_width)) % 2 == 0] = 255
rr.log("camera/image", rr.Image(checkerboard))

# Use a blueprint to show both 3D and 2D views side by side.
blueprint = rrb.Blueprint(
    rrb.Horizontal(
        # 3D view showing the scene and camera
        rrb.Spatial3DView(
            origin="world",
            name="3D Scene",
            contents=["/**"],
            overrides={
                # Adjust visual size of camera frustum in 3D view for better visibility.
                "camera": rr.Pinhole.from_fields(image_plane_distance=1.0)
            },
        ),
        # 2D projection from angled camera
        rrb.Spatial2DView(
            origin="camera",  # Make sure that the origin is at the camera's path.
            name="Camera",
            contents=["/**"],  # Add everything, so 3D objects get projected.
        ),
    )
)

rr.send_blueprint(blueprint)
