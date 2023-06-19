"""Create and log a segmentation image."""

import numpy as np
from PIL import Image
from PIL import ImageDraw

import rerun as rr

# Create a segmentation image with Pillow
width, height = 300, 200
image = Image.new("L", (width, height), color=0)
draw = ImageDraw.Draw(image)
draw.rectangle((50, 50, 100, 120), fill=1)
draw.ellipse((100, 130, 280, 180), fill=2)

rr.init("segmentation_image", spawn=True)

# Assign a label and color to each class
rr.log_annotation_context(
    "/",
    [
        (1, "rect", (255, 0, 0)),
        (2, "ellipse", (0, 255, 0)),
    ],
)


rr.log_segmentation_image("image", np.array(image))
