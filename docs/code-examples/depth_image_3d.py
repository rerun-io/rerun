"""Create and log a depth image and pinhole camera."""

import numpy as np
import rerun as rr

# Create a dummy depth image
image = 65535 * np.ones((200, 300), dtype=np.uint16)
image[50:150, 50:150] = 20000
image[130:180, 100:280] = 45000


rr.init("depth_image", spawn=True)

# If we log a pinhole camera model, the depth gets automatically back-projected to 3D
focal_length = 200
rr.log_pinhole(
    "world/camera",
    child_from_parent=np.array(
        (
            (focal_length, 0, image.shape[1] / 2),
            (0, focal_length, image.shape[0] / 2),
            (0, 0, 1),
        ),
    ),
    width=image.shape[1],
    height=image.shape[0],
)

# Log the tensor, assigning names to each dimension
rr.log_depth_image("world/camera/depth", image, meter=10000.0)
