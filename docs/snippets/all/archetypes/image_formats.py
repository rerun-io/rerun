"""Create and log an image with various formats."""

import numpy as np
import rerun as rr

rr.init("rerun_example_image_formats", spawn=True)

# Simple gradient image, logged in different formats.
image = np.array([[[x, min(255, x + y), y] for x in range(0, 256)] for y in range(0, 256)], dtype=np.uint8)
rr.log("image_rgb", rr.Image(image))
rr.log("image_green_only", rr.Image(image[:, :, 1], color_model="l"))  # Luminance only
rr.log("image_bgr", rr.Image(image[:, :, ::-1], color_model="bgr"))  # BGR

# New image with Separate Y/U/V planes with 4:2:2 chroma downsampling
y = bytes([128 for y in range(0, 256) for x in range(0, 256)])
u = bytes([x * 2 for y in range(0, 256) for x in range(0, 128)])  # Half horizontal resolution for chroma.
v = bytes([y for y in range(0, 256) for x in range(0, 128)])
rr.log("image_yuv422", rr.Image(bytes=y + u + v, width=256, height=256, pixel_format=rr.PixelFormat.Y_U_V16_FullRange))
