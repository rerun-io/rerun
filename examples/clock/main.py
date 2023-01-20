#!/usr/bin/env python3

"""An example showing usage of `log_arrow`.

An analog clock is built with Rerun Arrow3D primitives.
"""

import argparse
import math
from typing import Final, Tuple

import numpy as np

import rerun as rr

LENGTH_S: Final = 20.0
LENGTH_M: Final = 10.0
LENGTH_H: Final = 4.0

WIDTH_S: Final = 0.25
WIDTH_M: Final = 0.5
WIDTH_H: Final = 0.7


def log_clock(steps: int) -> None:
    def rotate(angle: float, len: float) -> Tuple[float, float, float]:
        return (
            len * math.sin(angle),
            len * math.cos(angle),
            0.0,
        )

    rr.log_view_coordinates("world", up="+Y", timeless=True)

    rr.log_obb(
        "world/frame",
        half_size=[2 * LENGTH_S, 2 * LENGTH_S, 1.0],
        position=[0.0, 0.0, 0.0],
        rotation_q=[0.0, 0.0, 0.0, 0.0],
        timeless=True,
    )

    for step in range(steps):
        t_secs = step

        rr.set_time_seconds("sim_time", t_secs)

        scaled_s = (t_secs % 60) / 60.0
        point_s = np.array(rotate(math.tau * scaled_s, LENGTH_S))
        color_s = (int(255 - (scaled_s * 255)), int(scaled_s * 255), 0, 128)
        rr.log_point("world/seconds_pt", position=point_s, color=color_s)
        rr.log_arrow("world/seconds_hand", origin=[0.0, 0.0, 0.0], vector=point_s, color=color_s, width_scale=WIDTH_S)

        scaled_m = (t_secs % 3600) / 3600.0
        point_m = np.array(rotate(math.tau * scaled_m, LENGTH_M))
        color_m = (int(255 - (scaled_m * 255)), int(scaled_m * 255), 128, 128)
        rr.log_point("world/minutes_pt", position=point_m, color=color_m)
        rr.log_arrow("world/minutes_hand", origin=[0.0, 0.0, 0.0], vector=point_m, color=color_m, width_scale=WIDTH_M)

        scaled_h = (t_secs % 43200) / 43200.0
        point_h = np.array(rotate(math.tau * scaled_h, LENGTH_H))
        color_h = (int(255 - (scaled_h * 255)), int(scaled_h * 255), 255, 255)
        rr.log_point("world/hours_pt", position=point_h, color=color_h)
        rr.log_arrow("world/hours_hand", origin=[0.0, 0.0, 0.0], vector=point_h, color=color_h, width_scale=WIDTH_M)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="An example visualizing an analog clock is built with Rerun Arrow3D primitives."
    )
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--steps", type=int, default=10_000, help="The number of time steps to log")
    args = parser.parse_args()

    rr.init("clock", is_official_example=True)

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    log_clock(args.steps)

    if args.save is not None:
        rr.save(args.save)
