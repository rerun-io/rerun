from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Drag-and-drop selection

The goal of this test is to test the selection behavior of drag-and-drop.

#### View selects on a successful drop

1. Select the `cos_curve` entity in the streams tree.
2. Drag it to the PLOT view and drop it.
3. _Expect_: the entity is added to the view, and the view becomes selected.


#### View doesn't select on a failed drop

1. Select the `cos_curve` entity again.
2. Drag it to the PLOT view (it should be rejected) and drop it.
3. _Expect_: nothing happens, and the selection is not changed.


#### Dragging an unselected item doesn't change the selection

1. Select the PLOT view.
2. Drag drag the `line_curve` entity to the PLOT view and drop it.
2. _Expect_:
    - The selection remains unchanged (the PLOT view is still selected).
    - The `line_curve` entity is added to the view.

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TimeSeriesView(origin="/", contents=[], name="PLOT"),
        rrb.TextDocumentView(origin="readme"),
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
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_some_scalar_entities()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
