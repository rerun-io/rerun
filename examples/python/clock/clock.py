#!/usr/bin/env python3
"""
An example showing usage of `log_arrow`.

An analog clock is built with Rerun Arrow3D primitives.
"""

from __future__ import annotations

import argparse
import math
from typing import Final

import numpy as np
import rerun as rr  # pip install rerun-sdk

LENGTH_S: Final = 20.0
LENGTH_M: Final = 10.0
LENGTH_H: Final = 4.0

WIDTH_S: Final = 0.125
WIDTH_M: Final = 0.2
WIDTH_H: Final = 0.3


def log_clock(steps: int) -> None:
    def rotate(angle: float, len: float) -> tuple[float, float, float]:
        return (
            len * math.sin(angle),
            len * math.cos(angle),
            0.0,
        )

    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Y_UP, static=True)

    rr.log(
        "world/frame",
        rr.Boxes3D(half_sizes=[LENGTH_S, LENGTH_S, 1.0], centers=[0.0, 0.0, 0.0]),
        static=True,
    )

    for step in range(steps):
        t_secs = step

        rr.set_time("sim_time", timedelta=t_secs)

        scaled_s = (t_secs % 60) / 60.0
        point_s = np.array(rotate(math.tau * scaled_s, LENGTH_S))
        color_s = (int(255 - (scaled_s * 255)), int(scaled_s * 255), 0, 128)
        rr.log("world/seconds_pt", rr.Points3D(positions=point_s, colors=color_s))
        rr.log("world/seconds_hand", rr.Arrows3D(vectors=point_s, colors=color_s, radii=WIDTH_S))

        scaled_m = (t_secs % 3600) / 3600.0
        point_m = np.array(rotate(math.tau * scaled_m, LENGTH_M))
        color_m = (int(255 - (scaled_m * 255)), int(scaled_m * 255), 128, 128)
        rr.log("world/minutes_pt", rr.Points3D(positions=point_m, colors=color_m))
        rr.log("world/minutes_hand", rr.Arrows3D(vectors=point_m, colors=color_m, radii=WIDTH_M))

        scaled_h = (t_secs % 43200) / 43200.0
        point_h = np.array(rotate(math.tau * scaled_h, LENGTH_H))
        color_h = (int(255 - (scaled_h * 255)), int(scaled_h * 255), 255, 255)
        rr.log("world/hours_pt", rr.Points3D(positions=point_h, colors=color_h))
        rr.log("world/hours_hand", rr.Arrows3D(vectors=point_h, colors=color_h, radii=WIDTH_H))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="An example visualizing an analog clock is built with Rerun Arrow3D primitives.",
    )
    parser.add_argument("--steps", type=int, default=10_000, help="The number of time steps to log")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_clock")
    log_clock(args.steps)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
