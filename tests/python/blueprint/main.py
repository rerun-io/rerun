from __future__ import annotations

import rerun as rr
from numpy.random import default_rng
from rerun.blueprint import (
    Blueprint,
    Grid,
    Horizontal,
    Spatial2DView,
    Spatial3DView,
    Tabs,
    TimePanel,
    Vertical,
)

if __name__ == "__main__":
    blueprint = Blueprint(
        Vertical(
            Spatial3DView(origin="/test1"),
            Horizontal(
                Tabs(
                    Spatial3DView(origin="/test1"),
                    Spatial2DView(origin="/test2"),
                ),
                Grid(
                    Spatial3DView(origin="/test1"),
                    Spatial2DView(origin="/test2"),
                    Spatial3DView(origin="/test1"),
                    Spatial2DView(origin="/test2"),
                    grid_columns=3,
                    column_shares=[1, 1, 1],
                ),
                column_shares=[1, 2],
            ),
            row_shares=[2, 1],
        ),
        TimePanel(expanded=False),
    )

    rr.init(
        "rerun_example_blueprint_test",
        spawn=True,
        default_blueprint=blueprint,
    )

    rng = default_rng(12345)
    positions = rng.uniform(-5, 5, size=[10, 3])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("test1", rr.Points3D(positions, colors=colors, radii=radii))
    rr.log("test2", rr.Points2D(positions[:, :2], colors=colors, radii=radii))
