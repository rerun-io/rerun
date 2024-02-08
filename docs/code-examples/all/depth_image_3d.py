"""Create and log a depth image and pinhole camera."""

import numpy as np
import rerun as rr

depth_image = 65535 * np.ones((200, 300), dtype=np.uint16)
depth_image[50:150, 50:150] = 20000
depth_image[130:180, 100:280] = 45000

rr.init("rerun_example_depth_image_3d", spawn=True)

# If we log a pinhole camera model, the depth gets automatically back-projected to 3D
rr.log(
    "world/camera",
    rr.Pinhole(
        width=depth_image.shape[1],
        height=depth_image.shape[0],
        focal_length=200,
    ),
)

# Log the tensor.
rr.log("world/camera/depth", rr.DepthImage(depth_image, meter=10_000.0))
