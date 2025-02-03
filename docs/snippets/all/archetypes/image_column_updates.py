"""
Update an image over time, in a single operation.

This is semantically equivalent to the `image_row_updates` example, albeit much faster.
"""

import numpy as np
import rerun as rr

rr.init("rerun_example_image_column_updates", spawn=True)

# Timeline on which the images are distributed.
times = np.arange(0, 20)

# Create a batch of images with a moving rectangle.
width, height = 300, 200
images = np.zeros((len(times), height, width, 3), dtype=np.uint8)
images[:, :, :, 2] = 255
for t in times:
    images[t, 50:150, (t * 10) : (t * 10 + 100), 1] = 255

# Log the ImageFormat and indicator once, as static.
format = rr.components.ImageFormat(width=width, height=height, color_model="RGB", channel_datatype="U8")
rr.log("images", rr.Image.from_fields(format=format), static=True)

# Send all images at once.
rr.send_columns(
    "images",
    indexes=[rr.TimeSequenceColumn("step", times)],
    # Reshape the images so `Image` can tell that this is several blobs.
    #
    # Note that the `Image` consumes arrays of bytes, so if you have a different channel
    # datatype than `U8`, you need to make sure that the data is converted to arrays of bytes
    # before passing it to `Image`.
    columns=rr.Image.columns(buffer=images.reshape(len(times), -1)),
)
