from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """
# 3D & 2D Heuristics

This checks whether the heuristics do the right thing with mixed 2D and 3D data.

Reset the blueprint to make sure you are viewing new heuristics and not a cached blueprint.

### Action
You should see 4 space-views:
 - 2D: `image1` with an all red image
 - 2D: `image2` with an all green image
 - 2D: `3d/camera` with an all blue image
 - 3D: `3d` with:
    - a 3D box
    - a pinhole camera, showing the blue image
    - no red or green image
"""


def log_image(path: str, height: int, width: int, color: tuple[int, int, int]) -> None:
    image = np.zeros((height, width, 3), dtype=np.uint8)
    image[:, :, :] = color
    rr.log(path, rr.Image(image))


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_images() -> None:
    log_image("image1", 20, 30, (255, 0, 0))
    log_image("image2", 20, 30, (0, 255, 0))


def log_3d_scene() -> None:
    rr.log("3d", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN)
    rr.log("3d/box", rr.Boxes3D(half_sizes=[1.0, 1.0, 1.0]))
    rr.log("3d/camera", rr.Pinhole(focal_length=30, width=30, height=20))
    log_image("3d/camera", 20, 30, (0, 0, 255))


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_images()
    log_3d_scene()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
