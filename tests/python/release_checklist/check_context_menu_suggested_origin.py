from __future__ import annotations

import math
import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """
# Context Menu - Test the origin selection heuristics

In the streams view, expend all entities in the `/world` hierarchy and create a spatial space view for every entity. Check the origin is as per the following table:

```plaintext
ENTITY                  CLASS   ORIGIN

/world                  3D      /
/world/camera           3D      /
/world/camera/image     3D      /
/world/camera/keypoint  3D      /

/world                  2D      <not suggested>
/world/camera           2D      <not suggested>
/world/camera/image     2D      /world/camera/image
/world/camera/keypoint  2D      /world/camera/image
```
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)
    rr.log("/world", rr.Boxes3D(centers=[0, 0, 0], half_sizes=[1, 1, 1]))
    rr.log(
        "/world/camera/image",
        # rr.Pinhole(fov_y=0.7853982, aspect_ratio=1, camera_xyz=rr.ViewCoordinates.RUB, resolution=[10, 10]),
        rr.Pinhole(
            resolution=[10, 10],
            focal_length=[4, 4],
            principal_point=[5, 5],
        ),
    )
    rr.log("/world/camera/image", rr.Image(np.random.rand(10, 10, 3)))
    rr.log("/world/camera/image/keypoint", rr.Points2D(np.random.rand(10, 2) * 10, radii=0.5))

    for i in range(100):
        rr.set_time_sequence("frame_nr", i)
        angle = 2 * math.pi * i / 100

        rr.log(
            "/world/camera",
            rr.Transform3D(
                rr.TranslationRotationScale3D(
                    translation=[math.cos(angle), math.sin(angle), 0],
                    rotation=rr.RotationAxisAngle(axis=[0, 0, 1], angle=angle),
                )
            ),
        )


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
