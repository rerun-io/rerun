from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Out-of-tree data results

[Background issue](https://github.com/rerun-io/rerun/issues/5742)

* Expand all the "TEST" view.
* Check that you can select each of its children.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Blueprint(
        rrb.Horizontal(
            rrb.TextDocumentView(origin="readme"),
            rrb.Spatial3DView(name="TEST", origin="/", contents="$origin/box/points/**"),
        )
    )


def log_data() -> None:
    rr.log("/", rr.Transform3D(translation=[1, 0, 0]))
    rr.log("/box", rr.Boxes3D(centers=[[0, 0, 0]], half_sizes=[0.5, 1, 0.5]))
    rr.log("/box", rr.Transform3D(translation=[0, 1, 0]))
    rr.log("/box/points", rr.Points3D(positions=[[0, 0, 0], [1, 1, 1]], radii=0.3))


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
