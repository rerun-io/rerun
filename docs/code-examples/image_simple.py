"""Create and log an image."""

import numpy as np
from PIL import Image, ImageDraw
import rerun as rr

# Create an image with Pillow
image = Image.new("RGB", (300, 200), color=(255, 0, 0))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((50, 50, 150, 150), fill=(0, 255, 0), radius=20)

rr.init("images", spawn=True)

# Pillow image have built-in support for Numpy conversion and use an
# RGB(A) ordering that is compatible with Rerun.
rr.log_image("simple", np.array(image))
