from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Entity drag-and-drop

The goal of this test is to check the behavior of views when entities are dragged-and-dropped over them.
The drag payload pill _and_ the mouse cursor should change based on the underlying UI element feedback.
This table summarizes the three possible states:

**Note**: actual cursor shape may vary depending on the platform (OS, browser, etc).

| **Ignore** | **Accept** | **Reject** |
| --- | --- | --- |
| Gray pill | Blue pill | Gray pill |
| Hand cursor | Hand cursor | No-drop cursor |
| ![ignore](https://static.rerun.io/dnd-cursor-ignore/ec48f64a119bddd2c9cbd55410021ef0e1a30feb/full.png) | ![accept](https://static.rerun.io/dnd-cursor-accept/7b40cd79fd99ba2c31617d2f40f56c5c8ba3aca0/full.png) | ![reject](https://static.rerun.io/dnd-cursor-reject/6f105e9689be33b2e0fff5bb1ad42cbb6271b622/full.png) |


#### Ignore state

1. Drag any entity from the streams tree over the blueprint tree (which ignores entities).
2. _Expect_: pill/cursor in ignore state, nothing happens on drop


#### Accept state

1. Drag the `cos_curve` entity from the streams tree to the BOTTOM view and drop it.
2. _Expect_: pill/cursor in accept state, entity is added to the BOTTOM view on drop.


#### Reject state

1. Drag THE SAME `cos_curve` entity from the streams tree.
2. _Expect_:
    - BOTTOM rejects the entity, nothing happens on drop.
    - TOP accepts the entity, entity is added to the TOP view on drop.

#### Multi-selection drag

1. Multi-select `cos_curve` and `line_curve` entities from the streams tree.
2. _Expect_: both views accept the entities, only `line_curve` is added on drop.

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.Vertical(
            rrb.TimeSeriesView(origin="/", contents=[], name="TOP"),
            rrb.TimeSeriesView(origin="/", contents=[], name="BOTTOM"),
        ),
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
