from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Data override check

- Verify that you can set a default point radius
- Verify that you can override point color
- Verify that you see three points are visible
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()

    # Logged as static because https://github.com/rerun-io/rerun/pull/7199
    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]), static=True)
    # Log it again, to ensure that the newest one is visible
    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1], [2, 2, 2]]), static=True)

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
