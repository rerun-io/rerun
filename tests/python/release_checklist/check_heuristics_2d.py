from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """\
# 2D Heuristics

This checks whether the heuristics do the right thing with images.

Reset the blueprint to make sure you are viewing new heuristics and not a cached blueprint.

### Action
You should see 4 views. Depending on timing you may end up with a 5th view at the root.
This should go away when you reset.

The four remaining views should be:
 - `image1` with a red square
 - `image2` with a green square
 - `image3` with a green square, which when hovered shows two popups:
    - green square
    - blue square
 - `segmented` with a red square and overlapping green square (rendered yellow)
"""


def log_image(path: str, height: int, width: int, color: tuple[int, int, int]) -> None:
    image = np.zeros((height, width, 3), dtype=np.uint8)
    image[:, :, :] = color
    rr.log(path, rr.Image(image))


def log_image_nested(path: str, height: int, width: int, color: tuple[int, int, int]) -> None:
    image = np.zeros((height, width, 3), dtype=np.uint8)
    image[int(height / 4) : int(height - height / 4), int(width / 4) : int(width - width / 4), :] = color
    rr.log(path, rr.Image(image))


def log_annotation_context() -> None:
    rr.log("/", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), static=True)


def log_segmentation(path: str, height: int, width: int, class_id: int) -> None:
    image = np.zeros((height, width, 1), dtype=np.uint8)
    image[int(height / 4) : int(height - height / 4), int(width / 4) : int(width - width / 4), 0] = class_id
    rr.log(path, rr.SegmentationImage(image))


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_images() -> None:
    log_annotation_context()
    log_image("image1", 20, 30, (255, 0, 0))
    log_image("image2", 20, 30, (0, 255, 0))
    log_image("image3", 20, 30, (0, 0, 255))
    log_image_nested("image3/nested", 20, 30, (0, 255, 0))
    log_image("segmented/image4", 20, 30, (255, 0, 0))
    log_segmentation("segmented/seg", 20, 30, 2)


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_images()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
