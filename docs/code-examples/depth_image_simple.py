"""Create and log a depth image."""

import numpy as np
import rerun as rr
import rerun.experimental as rr2

# Create a dummy depth image
image = 65535 * np.ones((8, 12), dtype=np.uint16)
image[0:4, 0:6] = 20000
image[4:8, 6:12] = 45000


rr.init("rerun_example_depth_image", spawn=True)

# Log the tensor, assigning names to each dimension
rr2.log("depth", rr2.DepthImage(image, meter=10_000.0))
