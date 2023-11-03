"""Log an image."""

import tempfile

import cv2
import numpy as np
import rerun as rr
from PIL import Image, ImageDraw

# Save a transparent PNG to a temporary file.
_, file_path = tempfile.mkstemp(suffix=".png")
image = Image.new("RGBA", (300, 200), color=(0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rectangle((0, 0, 300, 200), outline=(255, 0, 0), width=6)
draw.rounded_rectangle((50, 50, 150, 150), fill=(0, 255, 0), radius=20)
image.save(file_path)


rr.init("rerun_example_images_adv", spawn=True)

# Log the image from the file.
rr.log("from_file", rr.ImageEncoded(path=file_path))

# Read with Pillow and NumPy, and log the image.
image = np.array(Image.open(file_path))
rr.log("from_pillow_rgba", rr.Image(image))

# Convert to RGB, fill transparent pixels with a color, and log the image.
image_rgb = image[..., :3]
image_rgb[image[:, :, 3] == 0] = (45, 15, 15)
rr.log("from_pillow_rgb", rr.Image(image_rgb))

# Read with OpenCV
image = cv2.imread(file_path)

# OpenCV uses BGR ordering, so we need to convert to RGB.
image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
rr.log("from_opencv", rr.Image(image))
