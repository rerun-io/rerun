from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Hover, Select, Deselect, and Reset

This checks whether different UIs behave correctly with hover and selection.

### Hover
For each of the views:
* Hover the view and verify it shows up as highlighted in the blueprint tree.
* Hover the entity and verify it shows up highlighted in the blueprint tree.
  * For 2D and 3D views the entity itself should be outlined and show a hover element.
  * For plot view, the line-series will not highlight, but the plot should show info about the point.

### 2D/3D Select
For each of the views:
* Click on the background of the view, and verify the view becomes selected.
* Click on an entity, and verify the it becomes selected.
  * For 2D and 3D views the selected instance will not be visible in the blueprint tree.
    * If you think this is unexpected, create an issue.
    * Double-click the entity and verify that it becomes selected and highlighted in the blueprint tree.

### Graph Select
Should work just as 2D/3D views.

### Text view
Clicking on a text view (what you're reading right now) should select the view.
Hovering the view should work as well.

### Reset
For each of the views:
* Zoom and/or pan the view
* Double-click the background of the view and verify it resets the view to its default state.

### Deselect
Finally, try hitting escape and check whether that deselects whatever was currently selected and the recording is
selected instead.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_plots() -> None:
    from math import cos, sin, tau

    rr.log("plots/cos", rr.SeriesPoint())

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.Scalar(sin_of_t))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.Scalar(cos_of_t))


def log_points_3d() -> None:
    from numpy.random import default_rng

    rng = default_rng(12345)

    positions = rng.uniform(-5, 5, size=[10, 3])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("3D/points", rr.Points3D(positions, colors=colors, radii=radii))


def log_points_2d() -> None:
    from numpy.random import default_rng

    rng = default_rng(12345)

    positions = rng.uniform(-5, 5, size=[10, 2])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("2D/points", rr.Points2D(positions, colors=colors, radii=radii))


def log_graph() -> None:
    rr.log("graph", rr.GraphNodes(["a", "b"], labels=["A", "B"]))


def log_map() -> None:
    rr.log("points", rr.GeoPoints(lat_lon=[[47.6344, 19.1397], [47.6334, 19.1399]], radii=rr.Radius.ui_points(20.0)))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_plots()
    log_points_3d()
    log_points_2d()
    log_graph()
    log_map()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
