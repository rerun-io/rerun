#!/usr/bin/env python3

"""An example showing usage of `log_arrow`.

An analog clock is built with Rerun Arrow3D primitives.
"""

import argparse
import math
from typing import Final, Tuple

import numpy as np
import rerun_sdk as rerun

LENGTH_S: Final = 20.0
LENGTH_M: Final = 10.0
LENGTH_H: Final = 4.0

WIDTH_S: Final = 0.25
WIDTH_M: Final = 0.5
WIDTH_H: Final = 0.7


def log_clock() -> None:
    def rotate(theta: float, len: float) -> Tuple[float, float, float]:
        return (
            math.cos(theta) - len * math.sin(theta),
            math.sin(theta) + len * math.cos(theta),
            0.0,
        )

    rerun.log_view_coordinates("3d", up="+Y", timeless=True)

    rerun.log_obb(
        "3d/frame",
        half_size=[2 * LENGTH_S, 2 * LENGTH_S, 1.0],
        position=[0.0, 0.0, 0.0],
        rotation_q=[0.0, 0.0, 0.0, 0.0],
        timeless=True,
    )

    for t_secs in range(12 * 60 * 60):
        # Speed things up a little, set 1sec = 1min
        rerun.set_time_seconds("sim_time", t_secs / 60)

        scaled_s = (t_secs % 60) / 60.0
        point_s = np.array(rotate(2 * math.pi * scaled_s, LENGTH_S))
        color_s = (int(255 - (scaled_s * 255)), int(scaled_s * 255), 0, 128)
        rerun.log_point("3d/seconds_pt", position=point_s, color=color_s)
        rerun.log_arrow("3d/seconds_hand", origin=[0.0, 0.0, 0.0], vector=point_s, color=color_s, width_scale=WIDTH_S)

        scaled_m = (t_secs % 3600) / 3600.0
        point_m = np.array(rotate(2 * math.pi * scaled_m, LENGTH_M))
        color_m = (int(255 - (scaled_m * 255)), int(scaled_m * 255), 128, 128)
        rerun.log_point("3d/minutes_pt", position=point_m, color=color_m)
        rerun.log_arrow("3d/minutes_hand", origin=[0.0, 0.0, 0.0], vector=point_m, color=color_m, width_scale=WIDTH_M)

        scaled_h = (t_secs % 43200) / 43200.0
        point_h = np.array(rotate(2 * math.pi * scaled_h, LENGTH_H))
        color_h = (int(255 - (scaled_h * 255)), int(scaled_h * 255), 255, 255)
        rerun.log_point("3d/hours_pt", position=point_h, color=color_h)
        rerun.log_arrow("3d/hours_hand", origin=[0.0, 0.0, 0.0], vector=point_h, color=color_h, width_scale=WIDTH_M)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--download", action="store_true", help="Download dataset")
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    log_clock()

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()

    rerun.show()
