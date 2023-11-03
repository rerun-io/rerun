"""Playground to test the visible history feature."""
from __future__ import annotations

import argparse
import math

import numpy as np
import rerun as rr

parser = argparse.ArgumentParser(description=__doc__)
rr.script_add_args(parser)
args = parser.parse_args()
rr.script_setup(args, "rerun_example_visible_history_playground")


rr.log("bbox", rr.Boxes2D(centers=[50, 3.5], half_sizes=[50, 4.5], colors=[255, 0, 0]), timeless=True)
rr.log("transform", rr.Transform3D(translation=[0, 0, 0]))
rr.log("some/nested/pinhole", rr.Pinhole(focal_length=3, width=3, height=3), timeless=True)

rr.log("3dworld/depthimage/pinhole", rr.Pinhole(focal_length=20, width=100, height=10), timeless=True)
rr.log("3dworld/image", rr.Transform3D(translation=[0, 1, 0]), timeless=True)
rr.log("3dworld/image/pinhole", rr.Pinhole(focal_length=20, width=100, height=10), timeless=True)

for i in range(1, 100):
    rr.set_time_seconds("seconds", i)
    rr.set_time_seconds("ms", i / 1000)
    rr.set_time_seconds("us", i / 1000000)
    rr.set_time_sequence("frames_offset", 10000 + i)
    rr.set_time_sequence("frames", i)

    rr.log("world/data/nested/point", rr.Points2D([[i, 0], [i, 1]], radii=0.4))
    rr.log("world/data/nested/point2", rr.Points2D([i, 2], radii=0.4))
    rr.log("world/data/nested/box", rr.Boxes2D(centers=[i, 1], half_sizes=[0.5, 0.5]))
    rr.log("world/data/nested/arrow", rr.Arrows3D(origins=[i, 4, 0], vectors=[0, 1.7, 0]))
    rr.log(
        "world/data/nested/linestrip",
        rr.LineStrips2D([[[i - 0.4, 6], [i + 0.4, 6], [i - 0.4, 7], [i + 0.4, 7]], [[i - 0.2, 6.5], [i + 0.2, 6.5]]]),
    )

    rr.log("world/data/nested/transformed", rr.Transform3D(translation=[i, 0, 0]))
    rr.log("world/data/nested/transformed/point", rr.Boxes2D(centers=[0, 3], half_sizes=[0.5, 0.5]))

    rr.log("text_log", rr.TextLog(f"hello {i}"))
    rr.log("scalar", rr.TimeSeriesScalar(math.sin(i / 100 * 2 * math.pi)))

    depth_image = 100 * np.ones((10, 100), dtype=np.float32)
    depth_image[:, i] = 50
    rr.log("3dworld/depthimage/pinhole/data", rr.DepthImage(depth_image, meter=100))

    image = 100 * np.ones((10, 100, 3), dtype=np.uint8)
    image[:, i, :] = [255, 0, 0]
    rr.log("3dworld/image/pinhole/data", rr.Image(image))

    x_coord = (i - 50) / 5
    rr.log(
        "3dworld/mesh",
        rr.Mesh3D(
            vertex_positions=[[x_coord, 2, 0], [x_coord, 2, 1], [x_coord, 3, 0]],
            vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
        ),
    )
