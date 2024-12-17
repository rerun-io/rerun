from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Multi-entity drag-and-drop

This test checks that dragging multiple entities to a view correctly adds all entities.

1. Multi-select `cos_curve` and `line_curve` entities in the streams tree.
2. Drag them to the PLOT view.
3. _Expect_: both entities are visible in the plot view and each are listed in the view's entity path filter.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Vertical(
        rrb.TextDocumentView(origin="readme"),
        rrb.TimeSeriesView(origin="/", contents=[], name="PLOT"),
    )


def log_some_scalar_entities() -> None:
    times = np.arange(100)
    curves = [
        ("cos_curve", np.cos(times / 100 * 2 * np.pi)),
        ("line_curve", times / 100 + 0.2),
    ]

    time_column = rr.TimeSequenceColumn("frame", times)

    for path, curve in curves:
        rr.send_columns(path, times=[time_column], components=[rr.components.ScalarBatch(curve)])


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_default=True, make_active=True)

    log_readme()
    log_some_scalar_entities()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
