# Log data with various numbers of instances for test purposes, e.g. UI test, joining, etc.

from __future__ import annotations

import argparse
import random
import math

import numpy as np

import rerun as rr


def log_small_point_clouds(seed: int | None = None) -> None:
    """Logs a time-varying point cloud with often partial color information."""
    if seed is not None:
        random.seed(seed)
        np.random.seed(seed)

    N = 1000

    for i in range(N):
        num_points = random.randint(1, 20)
        num_colors = random.randint(1, num_points)
        x = i / N * 2
        y = math.sin(i / N * 2 * math.pi)
        spread = random.random() + 0.2

        points_x = np.ones(num_points) * x
        points_y = np.linspace(y - spread, y + spread, num_points)

        rr.log(
            "small_point_cloud/pts",
            rr.Points2D(
                positions=np.vstack([points_x, points_y]).T,
                radii=-4,
                colors=np.random.randint(0, 255, size=(num_colors, 4)),
            ),
        )

    rr.log("small_point_cloud/bbox", rr.Boxes2D(centers=[[1, 0]], sizes=[[3, 5]]), static=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Log some data with varying number of instances for testing purposes")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_instance_count_test")

    log_small_point_clouds()


if __name__ == "__main__":
    main()
