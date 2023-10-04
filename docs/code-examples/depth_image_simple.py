"""Create and log a depth image."""

import numpy as np
import rerun as rr

depth_image = 65535 * np.ones((8, 12), dtype=np.uint16)
depth_image[0:4, 0:6] = 20000
depth_image[4:8, 6:12] = 45000

rr.init("rerun_example_depth_image", spawn=True)

# Log the tensor, assigning names to each dimension
rr.log("depth", rr.DepthImage(depth_image, meter=10_000.0))
