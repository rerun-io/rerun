"""Create and log an image."""

from pathlib import Path

import rerun as rr

image_file_path = Path(__file__).parent / "../../../../crates/viewer/re_ui/data/logo_dark_mode.png"

rr.init("rerun_example_image_encoded", spawn=True)

rr.log("image", rr.ImageEncoded(path=image_file_path))
