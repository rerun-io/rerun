from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Mixed objects Heuristics with everything directly under root

This checks whether the heuristics do the right thing with mixed 2D, 3D and test data.

### Action
You should see 4 views:
 - 2D: Boxes and points, `$origin` == `/`
 - 3D: 3D boxes, `$origin` == `/`
 - Text log: A single log line, `$origin` == `/`
 - Text: This readme, `$origin` == `/readme`
"""


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))
    rr.log("text_logs", rr.TextLog("Hello, world!", level=rr.TextLogLevel.INFO))
    rr.log("points2d", rr.Points2D([[0, 0], [1, 1], [3, 2]], labels=["a", "b", "c"]))
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
