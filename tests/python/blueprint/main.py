from __future__ import annotations

import rerun as rr
from numpy.random import default_rng
from rerun.blueprint import Horizontal, Spatial2D, Spatial3D, Tabs, Vertical, Viewport

if __name__ == "__main__":
    root = Vertical(
        Spatial3D(origin="/test1"),
        Horizontal(
            Tabs(
                Spatial3D(origin="/test1"),
                Spatial2D(origin="/test2"),
            ),
            Spatial2D(origin="/test2"),
        ),
    )
    viewport = Viewport(root)

    rr.init(
        "rerun_example_blueprint_test",
        spawn=True,
        blueprint=viewport.create_blueprint("rerun_example_blueprint_test"),
    )

    rng = default_rng(12345)
    positions = rng.uniform(-5, 5, size=[10, 3])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("test1", rr.Points3D(positions, colors=colors, radii=radii))
    rr.log("test2", rr.Points2D(positions[:, :2], colors=colors, radii=radii))
