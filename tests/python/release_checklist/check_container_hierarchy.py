from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Container Hierarchy

This checks that the container hierarchy behaves as expected.

### Prepare a hierarchy

TODO(ab): setup the container hierarchy with the blueprint API when available.

* Organize the views in a non-trivial hierarchy of containers.
* As a starting point, ensure that the hierarchy is "sane" (i.e. no leaf/single-child containers, etc.).


### Container creation and drag-and-drop

* Use the following method to create a bunch of containers:
    * `+` button in the Blueprint tree
    * `+` button in the Selection Panel (when a container is selected)
    * TODO(ab): from the context-menu in the Blueprint tree
* Create a bloated hierarchy with these (mostly empty) containers using drag-and-drop in the Blueprint tree.
* Check that a container refuses to be dropped into itself.
* Check that a container refuses to be dropped before/after the "Viewport" root container.
* Check that the destination container properly highlights when dragging something into it.
* Check that a Horizontal/Vertical container may not be nested to a container of the same kind.


### Edit containers

* Select a container and change its kind. Check that this is reflected in the Blueprint tree and the Viewport.


### Simplify the hierarchy

* Select on mid-tree container, and click "Simplify hierarchy" in the Selection Panel.
    * Its content should be clean of any empty containers.
    * The rest of the tree should be unaffected.
* Select the "Viewport" root container, and click "Simplify hierarchy" in the Selection Panel.
    * The entire tree should be clean of spurious empty container.


### Drag-and-drop in the viewport

TODO(ab): be _way_ more specific exact actions and expected outcomes when drag-and-dropping tiles.

* Check that the tiles may be resized by dragging boundaries in the Viewport.
* In the Viewport, drag tiles around to build a different container hierarchy.
* Check that no spurious empty containers are created.

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_some_views() -> None:
    from math import cos, sin, tau

    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))
    rr.log("text_logs", rr.TextLog("Hello, world!", level=rr.TextLogLevel.INFO))
    rr.log("points2d", rr.Points2D([[0, 0], [1, 1], [3, 2]], labels=["a", "b", "c"]))
    rr.log("points2d/bbx", rr.Boxes2D(centers=[1, 1], half_sizes=[3, 3]))

    rr.log("plots/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)"), timeless=True)
    rr.log("plots/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)"), timeless=True)

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.Scalar(sin_of_t))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.Scalar(cos_of_t))


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
