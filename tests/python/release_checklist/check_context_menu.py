from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """
# Context Menu

### Single-selection checks

* Reset the blueprint.
* Right-click on any space view and check for context menu content:
    - Hide
    - Remove
    - Move to new container
    - Clone
* Check both work as expected.
* Right-click on the viewport and check for context menu content:
    - Add Container
    - Add Space View
* Add a container via the context menu, check the container appears at then end of the list.
* Right-click on the container and check for context menu content:
    - Hide
    - Remove
    - Move to new container
    - Add Container
    - Add Space View


### Selection behavior

* Select a space view.
* Right-click on _another_ space view: the selection should switch to the newly-clicked space view.
* Select multiple space views.
* Right-click on one of the selected space view: the selection should remain the same and the context menu should appear.
* With multiple space views selected, right click on another space view that isn't yet selected. The selection should switch to the newly-clicked space view.


### Multi-selection checks

* Select multiple space views, right-click, and check for context menu content:
    - Hide
    - Remove
    - Move to new container
* Same as above, but with only containers selected.
* Same as above, but with both space views and containers selected.
* Same as above, but with the viewport selected as well. The context menu should be identical, but none of its actions should apply to the viewport.

### Invalid sub-container kind

* Single-select a horizontal container, check that it disallow adding an horizontal container inside it.
* Same for a vertical container.
* Single select a space view inside a horizontal container, check that it disallow moving to a new horizontal container.
* Same for a space view inside a vertical container.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))
    rr.log("text_logs", rr.TextLog("Hello, world!", level=rr.TextLogLevel.INFO))
    rr.log("points2d", rr.Points2D([[0, 0], [1, 1], [3, 2]], labels=["a", "b", "c"]))
    rr.log("points2d/bbx", rr.Boxes2D(centers=[1, 1], half_sizes=[3, 3]))


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
