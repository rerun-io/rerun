from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Context Menu - Add entity to new view

## Blueprint tree

* Right-click on Viewport and select "Collapse all". Check everything is collapsed by manually expending everything.
* Right-click on Viewport and select "Collapse all" and then "Expend all". Check everything is expanded.

## Streams tree

* Same as above, with the `world/` entity.


## Multi-selection

* Same as above, with both the viewport (blueprint tree) and `world/` (streams tree) selected.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TextDocumentView(origin="readme"),
        rrb.Vertical(
            rrb.Horizontal(
                rrb.Vertical(
                    rrb.Spatial3DView(origin="/"),
                )
            )
        ),
        column_shares=[2, 1],
    )


def log_some_views() -> None:
    rr.set_time_sequence("frame_nr", 0)
    rr.log("/", rr.Boxes3D(centers=[0, 0, 0], half_sizes=[1, 1, 1]))
    rr.log("/world/robot/arm/actuator/thing", rr.Boxes3D(centers=[0.5, 0, 0], half_sizes=[0.1, 0.1, 0.1]))


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
