from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# 2D views: Overrides, logged values, defaults, fallbacks

This checks that component and visualizer overrides behave as expected for 2D views.

Specifically that…:
* …Adding and removing overrides/defaults using the blueprint APIs work.
* …Adding and removing overrides/defaults using the UI work.
* …Rendering behaves properly.

---

First things firsts, check that the views in the first column render properly:
* View **1)** should look like this:
  - ![View 1](https://static.rerun.io/check_overrides_2d_view_1/5454441272b0abee8dbc8d7a342ecfc4036cbe9b/480w.png)
* View **2)** should look like this:
  - ![View 2](https://static.rerun.io/check_overrides_2d_view_2/2d6f9d0cba38b3697114c4108d5d99fc5b0fd174/480w.png)
* View **3)** should look like this:
  - NOTE: The color might differ if you're running on the web. That's fine.
  - ![View 3](https://static.rerun.io/check_overrides_2d_view_3/b1409c2862103a5bddce2233995aabb1307f3964/480w.png)

---

Then we'll modify view **1)**. The goal is to make it look exactly like the view on the top right:
* Double-click any of the arrows to select the arrow batch.
* Remove the extra `Points2D` visualizer.
* Remove all extra component overrides on the remaining `Arrows2D` visualizer.
* You should end up with the exact same view as the one on the right of view **1)**, which itself should look exactly like view **2)**.

---

Then we'll modify view **2)**. The goal is to make it look exactly like the view on the middle right:
* Double-click any of the arrows to select the arrow batch.
* Remove all extra component defaults on the `Arrows2D` visualizer.
* You should end up with the exact same view as the one on the right of view **2)**, which itself should look exactly like view **3)**.

---

Finally, we'll modify view **3)**. The goal is to make it look exactly like the view on the bottom right:
* Double-click any of the arrows to select the arrow batch.
* Add overrides to the view until you end up with the exact same view as the one on the right of view **3)**.
* The resulting view should look like this:
  - ![View final](https://static.rerun.io/check_overrides_2d_view_final/b65b95157891b35d3a333ebcf3286f2dceef228f/480w.png)

---

👏👏👏
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_boxes() -> None:
    rr.log(
        "arrows",
        rr.Arrows2D(origins=[[-2.0, 0.0], [0.0, 0.0], [2.0, 0.0]], vectors=[[-2.0, 1.0], [0.0, 2.0], [2.0, 1.0]]),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_boxes()

    visual_bounds = rrb.VisualBounds2D(x_range=[-5.5, 5.5], y_range=[-3.0, 3.0])
    overrides = {
        "arrows": [
            rrb.VisualizerOverrides([
                rrb.visualizers.Arrows2D,
                rrb.visualizers.Points2D,
            ]),
            rr.components.ColorBatch([[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
            rr.components.RadiusBatch([0.5, 0.25, 0.125]),
            rr.components.TextBatch(["BigRed", "MidGreen", "SmolBlue"]),
            rr.components.Position2DBatch([[-2.0, 1.5], [0.0, -0.5], [2.0, 0.75]]),
            rr.components.Vector2DBatch([[-2.0, 1.0], [0.0, 2.0], [2.0, 1.0]]),
        ]
    }
    defaults = [
        rr.components.ColorBatch([[255, 255, 0], [0, 255, 255], [255, 0, 255]]),
        rr.components.RadiusBatch([0.1, 0.2, 0.3]),
        rr.components.TextBatch(["TeenyYellow", "AverageCyan", "GigaPurple"]),
    ]

    blueprint = rrb.Blueprint(
        rrb.Grid(
            rrb.TextDocumentView(origin="readme", name="Instructions"),
            rrb.Vertical(
                rrb.Spatial2DView(
                    name="1) Overrides, logged values & defaults",
                    visual_bounds=visual_bounds,
                    overrides=overrides,
                    defaults=defaults,
                ),
                rrb.Spatial2DView(
                    name="2) Logged values & defaults",
                    visual_bounds=visual_bounds,
                    defaults=defaults,
                ),
                rrb.Spatial2DView(
                    name="3) Logged values only",
                    visual_bounds=visual_bounds,
                ),
            ),
            rrb.Vertical(
                rrb.Spatial2DView(
                    name="What you should get after removing overrides from 1)",
                    visual_bounds=visual_bounds,
                    defaults=defaults,
                ),
                rrb.Spatial2DView(
                    name="What you should get after removing defaults from 2)",
                    visual_bounds=visual_bounds,
                ),
                rrb.Spatial2DView(
                    name="What you should get after adding overrides & defaults to 3)",
                    visual_bounds=visual_bounds,
                    overrides={
                        "arrows": [
                            rrb.VisualizerOverrides([
                                rrb.visualizers.Arrows2D,
                                rrb.visualizers.Points2D,
                            ]),
                            rr.components.Color([255, 255, 255]),
                            rr.components.Radius(0.1),
                            rr.components.Text("Cerberus"),
                            rr.components.Position2D([0.0, 0.0]),
                        ]
                    },
                ),
            ),
            grid_columns=3,
            column_shares=[1, 1, 1],
        ),
    )
    rr.send_blueprint(blueprint, make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
