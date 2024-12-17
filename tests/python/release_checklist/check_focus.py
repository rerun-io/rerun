from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Focus checks

- Double-click on a box in the first view
    - check ONLY the corresponding view expands and scrolls
    - check the streams view expands and scrolls
- Double-click on the leaf "boxes3d" entity in the streams view, check both views expand (manual scrolling might be needed).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.Tabs(*[rrb.TextDocumentView(origin="readme") for _ in range(100)]),
        rrb.Vertical(rrb.Spatial3DView(origin="/", name="SV1"), rrb.Spatial3DView(origin="/", name="SV2")),
        column_shares=[1, 2],
    )


def log_some_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    for i in range(500):
        rr.log(f"a_entity_{i}", rr.AnyValues(empty=0))

    rr.log(
        "/objects/boxes/boxes3d",
        rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_some_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
