"""Create and log an image."""

import numpy as np
import rerun as rr

# Create an image with Pillow
image = np.zeros((200, 300, 3), dtype=np.uint8)
image[:, :, 0] = 255
image[50:150, 50:150] = (0, 255, 0)

rr.init("rerun-example-images", spawn=True)

rr.log_image("simple", np.array(image))
