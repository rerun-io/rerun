"""Create and log a depth image and pinhole camera."""

import numpy as np
import rerun as rr

# Create a dummy depth image
image = 65535 * np.ones((200, 300), dtype=np.uint16)
image[50:150, 50:150] = 20000
image[130:180, 100:280] = 45000


rr.init("rerun_example_depth_image", spawn=True)

# If we log a pinhole camera model, the depth gets automatically back-projected to 3D
rr.log_pinhole(
    "world/camera",
    width=image.shape[1],
    height=image.shape[0],
    focal_length_px=200,
)

# Log the tensor, assigning names to each dimension
rr.log_depth_image("world/camera/depth", image, meter=10000.0)
