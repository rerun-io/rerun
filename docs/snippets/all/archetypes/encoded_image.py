"""Create and log an image."""

from pathlib import Path

import rerun as rr

image_file_path = Path(__file__).parent / "ferris.png"

rr.init("rerun_example_encoded_image", spawn=True)

rr.log("image", rr.EncodedImage(path=image_file_path))
