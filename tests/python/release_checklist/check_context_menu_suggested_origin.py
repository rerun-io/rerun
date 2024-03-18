from __future__ import annotations

import math
import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """
# Context Menu - Test the origin selection heuristics

Repeat these steps for each of the following entities and space view class:
- right-click the entity (either in the blueprint or streams tree)
- select "Add to new Space View" and create the space view of the listed class
- check that the created space view has the expected origin
- delete the space view


check that for the given space view class, the resulting suggested origin is as expected.

```plaintext
===========================================================
ENTITY                      CLASS       EXPECTED ORIGIN
-----------------------------------------------------------
/                           3D          /
/world                      3D          /world
/world/camera               3D          /world
/world/camera/image         3D          /world
/world/camera/keypoint      3D          /world
-----------------------------------------------------------
/world                      2D          <not suggested>
/world/camera               2D          <not suggested>
/world/camera/image         2D          /world/camera/image
/world/camera/keypoint      2D          /world/camera/image
===========================================================
```
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Viewport(
        rrb.Horizontal(
            rrb.TextDocumentView(origin="readme"),
            rrb.Spatial3DView(origin="/", contents="", name="root entity"),
            column_shares=[2, 1],
        )
    )


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)
    rr.log("/", rr.Boxes3D(centers=[0, 0, 0], half_sizes=[1, 1, 1]))
    rr.log("/world", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, timeless=True)
    rr.log(
        "/world/camera/image",
        rr.Pinhole(
            resolution=[10, 10],
            focal_length=[4, 4],
            principal_point=[5, 5],
        ),
    )
    rr.log("/world/camera/image", rr.Image(np.random.rand(10, 10, 3)))
    rr.log("/world/camera/image/keypoint", rr.Points2D(np.random.rand(10, 2) * 10, radii=0.5))

    rr.log(
        "/world/camera",
        rr.Transform3D(
            rr.TranslationRotationScale3D(
                rotation=rr.RotationAxisAngle(axis=[0, 0, 1], angle=math.pi / 2),
            )
        ),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4(), blueprint=blueprint())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
