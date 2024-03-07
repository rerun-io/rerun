from __future__ import annotations

import os
import random
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """
# Context Menu - Add entity to new space view

* Reset the blueprint.
* Expend all space views and data result.
* Right-click on the `boxes3d` entity and select "Add to new space view" -> "3D". Check a new space view is created _and selected_ with the boxes3d entity and origin set to root.
* In each space view, right-click on the leaf entity, and check that "Add to new space view" recommends at least space views of the same kind.
* Select both the `boxes3d` entity and the `text_logs` entity. Check no space view is recommended (except Dataframe if enabled).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))
    rr.log("text_logs", rr.TextLog("Hello, world!", level=rr.TextLogLevel.INFO))
    rr.log("bars", rr.BarChart([1, 2, 3, 4, 5]))
    rr.log("tensor", rr.Tensor(np.random.rand(3, 4, 5)))

    for i in range(10):
        rr.set_time_sequence("frame_nr", i)
        rr.log("timeseries", rr.TimeSeriesScalar(random.randint(0, 100)))


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
