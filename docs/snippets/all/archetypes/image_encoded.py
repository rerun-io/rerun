"""Create and log an image."""

from pathlib import Path

import rerun as rr

current_file_dir = Path(__file__).parent
target_file_path = current_file_dir / "../../../../crates/viewer/re_ui/data/logo_dark_mode.png"
with open(target_file_path, "rb") as file:
    file_bytes = file.read()

rr.init("rerun_example_image_encoded", spawn=True)

rr.log("image", rr.ImageEncoded(file_bytes))
