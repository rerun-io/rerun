from __future__ import annotations

import os
import random
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Context Menu - Add entity to new view

#### Blueprint tree

* "Expand all" on the Vertical containers.
* Right-click on the `boxes3d` entity and select "Add to new view" -> "3D". Check a new view is created _and selected_ with the boxes3d entity and origin set to root.
* In each view, right-click on the leaf entity, and check that "Add to new view" recommends at least views of the same kind.
* Select both the `boxes3d` entity and the `text_logs` entity. Check no view is recommended (except Dataframe if enabled).

#### Streams tree

* Right-click on the `bars` entity and check that a Bar Plot view can successfully be created.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TextDocumentView(origin="readme"),
        rrb.Vertical(
            rrb.Spatial3DView(origin="/boxes3d"),
            rrb.Spatial2DView(origin="/boxes2d"),
            rrb.TextLogView(origin="/text_logs"),
            rrb.BarChartView(origin="/bars"),
            rrb.TensorView(origin="/tensor"),
        ),
        column_shares=[2, 1],
    )


def log_some_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))
    rr.log("text_logs", rr.TextLog("Hello, world!", level=rr.TextLogLevel.INFO))
    rr.log("bars", rr.BarChart([1, 2, 3, 4, 5]))
    rr.log("tensor", rr.Tensor(np.random.rand(3, 4, 5)))

    for i in range(10):
        rr.set_time_sequence("frame_nr", i)
        rr.log("timeseries", rr.Scalar(random.randint(0, 100)))


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
