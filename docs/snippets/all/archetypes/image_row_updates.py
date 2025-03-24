"""
Update an image over time.

See also the `image_column_updates` example, which achieves the same thing in a single operation.
"""

import numpy as np
import rerun as rr

rr.init("rerun_example_image_row_updates", spawn=True)

for t in range(20):
    rr.set_time("time", sequence=t)

    image = np.zeros((200, 300, 3), dtype=np.uint8)
    image[:, :, 2] = 255
    image[50:150, (t * 10) : (t * 10 + 100)] = (0, 255, 255)

    rr.log("image", rr.Image(image))
