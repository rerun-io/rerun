from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """
# Context Menu - Invalid sub-container kind

## Preparation

TODO(ab): automate this with blueprints

- Reset the blueprint
- Add a Horizontal container and a Vertical container in the viewport, and move one space view into each.


## Checks

* Single-select a horizontal container, check that it disallow adding a horizontal container inside it.
* Same for a vertical container.
* Single select a space view inside a horizontal container, check that it disallow moving to a new horizontal container.
* Same for a space view inside a vertical container.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))


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
