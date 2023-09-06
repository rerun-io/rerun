"""Create and log an image."""

import numpy as np
import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

# Create an image with numpy
image = np.zeros((200, 300, 3), dtype=np.uint8)
image[:, :, 0] = 255
image[50:150, 50:150] = (0, 255, 0)

rr.init("rerun_example_image_simple", spawn=True)

image = rrd.TensorData(array=image)
rr2.log("image", rr2.Image(image))
