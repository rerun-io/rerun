from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Static components

In the streams view, check the hover tooltips and selection panel for each `Position2D` components. They should both
display warnings/errors according to the following table:


```plaintext
==========================================================================
COMPONENT                       FEEDBACK
==========================================================================

static:Position2D               -

static_overwrite:Position2D     warning (orange): overridden 2 times

hybrid:Position2D               error (red): 12 events logged on timelines

==========================================================================
```
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_some_views() -> None:
    rr.log("static", rr.Points2D([(0, 0), (1, 1), (2, 2)]), static=True)

    # override static component
    rr.log("static_overwrite", rr.Points2D([(0, 0), (1, 1), (2, 2)]), static=True)
    rr.log("static_overwrite", rr.Points2D([(0, 0), (1, 1), (5, 2)]), static=True)
    rr.log("static_overwrite", rr.Points2D([(0, 0), (1, 1), (10, 2)]), static=True)

    # mixed time-full and static logs
    rr.log("hybrid", rr.Points2D([(0, 0), (1, 1), (2, 2)]), static=True)

    rr.set_time_seconds("time", 1.0)
    rr.log("hybrid", rr.Points2D([(0, 0), (1, 1), (2, 2)]))
    rr.set_time_seconds("time", 1.0)
    rr.log("hybrid", rr.Points2D([(0, 0), (1, 1), (2, 2)]))
    rr.set_time_seconds("time", 1.0)
    rr.log("hybrid", rr.Points2D([(0, 0), (1, 1), (2, 2)]))

    rr.disable_timeline("time")
    rr.set_time_seconds("other_time", 10.0)
    rr.log("hybrid", rr.Points2D([(0, 0), (1, 1), (2, 2)]))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_views()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
