from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """\
# Annotations

This checks whether annotations behave correctly

### Actions

There should be one view with an image and a batch of 2 rectangles.

The image should contain a red region and a green region.
There should be 1 red rectangle and 1 green rectangle.

Hover over each of the elements and confirm it shows the label as "red" or "green" as expected.

*Note*: when hovering the rectangles, a tooltip pertaining to the image will _also_ appear and indicate a label of "0".
This is expected as the image is black at this location.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_annotations() -> None:
    # Log an annotation context to assign a label and color to each class
    rr.log(
        "/",
        rr.AnnotationContext([(0, "black", (0, 0, 0)), (1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]),
        static=True,
    )

    # Log a batch of 2 rectangles with different `class_ids`
    rr.log("detections", rr.Boxes2D(mins=[[200, 50], [75, 150]], sizes=[[30, 30], [20, 20]], class_ids=[1, 2]))

    # Create a simple segmentation image

    image = np.zeros((200, 300), dtype=np.uint8)
    image[50:100, 50:120] = 1
    image[100:180, 130:280] = 2
    rr.log("segmentation/image", rr.SegmentationImage(image))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_annotations()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
