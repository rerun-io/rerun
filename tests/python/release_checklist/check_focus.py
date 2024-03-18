from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """
# Focus checks

## Preparation

TODO(ab): automate this with blueprints
TODO(ab): add lots of stuff via blueprint to make the tree more crowded and check scrolling

- Reset the blueprint
- Clone the 3D space view such as to have 2 of them.

## Checks

- Collapse all in the blueprint tree and the streams view
- Double-click on the box in the first space view
    - check corresponding space view expands and scrolls
    - check the streams view expands and scrolls
- Collapse all in the blueprint tree.
- Double-click on the leaf "boxes3d" entity in the streams view, check both space views expand.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log(
        "/objects/boxes/boxes3d",
        rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
