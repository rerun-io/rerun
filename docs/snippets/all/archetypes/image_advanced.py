"""Log an image."""

import tempfile

import cv2
import numpy as np
import rerun as rr
from PIL import Image as PILImage, ImageDraw

# Save a transparent PNG to a temporary file.
_, file_path = tempfile.mkstemp(suffix=".png")
image = PILImage.new("RGBA", (300, 200), color=(0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rectangle((0, 0, 300, 200), outline=(255, 0, 0), width=6)
draw.rounded_rectangle((50, 50, 150, 150), fill=(0, 255, 0), radius=20)
image.save(file_path)


rr.init("rerun_example_image_advanced", spawn=True)

# Log the image from the file.
rr.log("from_file", rr.EncodedImage(path=file_path))

# Read with Pillow and NumPy, and log the image.
image = np.array(PILImage.open(file_path))
rr.log("from_pillow_rgba", rr.Image(image))

# Drop the alpha channel from the image.
image_rgb = image[..., :3]
rr.log("from_pillow_rgb", rr.Image(image_rgb))

# Read with OpenCV.
image = cv2.imread(file_path)
# OpenCV uses BGR ordering, we need to make this known to Rerun.
rr.log("from_opencv", rr.Image(image, color_model="BGR"))
