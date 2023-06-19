"""Create and log a depth image."""

import numpy as np
import rerun as rr
from PIL import Image, ImageDraw

# Create a depth image with Pillow
width, height = 300, 200
image = Image.new("L", (width, height), color=255)
draw = ImageDraw.Draw(image)
draw.rectangle((50, 50, 150, 150), fill=100)
draw.ellipse((100, 130, 280, 180), fill=180)

rr.init("depth_image", spawn=True)

# We need a camera to register the depth image to
focal_length = 200
rr.log_pinhole(
    "world/camera",
    child_from_parent=np.array(
        (
            (focal_length, 0, width / 2),
            (0, focal_length, height / 2),
            (0, 0, 1),
        ),
        dtype=np.float32,
    ),
    width=width,
    height=height,
)

# Log the tensor, assigning names to each dimension
rr.log_depth_image("world/camera/depth", np.array(image).astype(np.int16), meter=100.0)
