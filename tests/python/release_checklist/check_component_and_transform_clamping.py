from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Component & transform clamping behavior

This test checks the clamping behavior of components & instance poses on boxes & spheres.

One view shows spheres, the other boxes.

For both you should see:
* 2x red (one bigger than the other)
* 1x green
* 2x blue (one bigger than the other)
* NO other boxes/spheres, in particular no magenta ones!
"""

rerun_obj_path = f"{os.path.dirname(os.path.realpath(__file__))}/../../assets/rerun.obj"


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Blueprint(
        rrb.Horizontal(
            rrb.TextDocumentView(origin="readme"),
            rrb.Spatial3DView(origin="spheres"),
            rrb.Spatial3DView(origin="boxes"),
        )
    )


def log_data() -> None:
    rr.log("/boxes/clamped_colors", rr.Boxes3D(half_sizes=[[1, 1, 1], [2, 2, 2]], colors=[[255, 0, 0]]))
    rr.log(
        "/boxes/ignored_colors",
        rr.Boxes3D(half_sizes=[[1, 1, 1]], centers=[[5, 0, 0]], colors=[[0, 255, 0], [255, 0, 255]]),
    )
    rr.log(
        "/boxes/more_transforms_than_sizes",
        rr.Boxes3D(half_sizes=[[1, 1, 1]], centers=[[0, 5, 0]], colors=[[0, 0, 255]]),
        rr.InstancePoses3D(scales=[[1, 1, 1], [2, 2, 2]]),
    )
    rr.log(
        "/boxes/no_primaries",
        rr.Boxes3D(half_sizes=[], centers=[[5, 0, 0]], colors=[[255, 0, 255]]),
        rr.InstancePoses3D(scales=[[1, 1, 1], [2, 2, 2]]),
    )
    # Same again but with spheres.
    rr.log("/spheres/clamped_colors", rr.Ellipsoids3D(half_sizes=[[1, 1, 1], [2, 2, 2]], colors=[[255, 0, 0]]))
    rr.log(
        "/spheres/ignored_colors",
        rr.Ellipsoids3D(half_sizes=[[1, 1, 1]], centers=[[5, 0, 0]], colors=[[0, 255, 0], [255, 0, 255]]),
    )
    rr.log(
        "/spheres/more_transforms_than_sizes",
        rr.Ellipsoids3D(half_sizes=[[1, 1, 1]], centers=[[0, 5, 0]], colors=[[0, 0, 255]]),
        rr.InstancePoses3D(scales=[[1, 1, 1], [2, 2, 2]]),
    )
    rr.log(
        "/spheres/no_primaries",
        rr.Ellipsoids3D(half_sizes=[], centers=[[5, 0, 0]], colors=[[255, 0, 255]]),
        rr.InstancePoses3D(scales=[[1, 1, 1], [2, 2, 2]]),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)
    log_readme()
    log_data()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
