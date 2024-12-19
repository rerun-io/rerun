from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Modal scrolling

* Select the 2D view
* Open the Entity Path Filter modal
* Make sure it behaves properly, including scrolling
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_many_entities() -> None:
    for i in range(0, 1000):
        rr.log(f"points/{i}", rr.Points2D([(i, i)]))


def run(args: Namespace) -> None:
    rr.script_setup(
        args,
        f"{os.path.basename(__file__)}",
        recording_id=uuid4(),
    )
    rr.send_blueprint(
        rrb.Grid(rrb.Spatial2DView(origin="/"), rrb.TextDocumentView(origin="readme")),
        make_active=True,
        make_default=True,
    )

    log_readme()
    log_many_entities()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
