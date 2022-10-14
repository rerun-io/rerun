#!/usr/bin/env python3
"""Minimal examples of Rerun SDK usage.

set_visible:
Uses `rerun.set_visible` to toggle the visibility of some rects
"""

import argparse
from math import pi
import math
from time import sleep
from typing import Any, Tuple

import rerun_sdk as rerun


def args_set_visible(subparsers: Any) -> None:
    set_visible_parser = subparsers.add_parser("set_visible")
    set_visible_parser.set_defaults(func=run_set_visible)


def run_set_visible(args: argparse.Namespace) -> None:
    rerun.set_time_seconds("sim_time", 1)
    rerun.log_rect("rect/0", [5, 5, 4, 4], label="Rect1", color=(255, 0, 0))
    rerun.log_rect("rect/1", [10, 5, 4, 4], label="Rect2", color=(0, 255, 0))
    rerun.set_time_seconds("sim_time", 2)
    rerun.set_visible("rect/0", False)
    rerun.set_time_seconds("sim_time", 3)
    rerun.set_visible("rect/1", False)
    rerun.set_time_seconds("sim_time", 4)
    rerun.set_visible("rect/0", True)
    rerun.set_time_seconds("sim_time", 5)
    rerun.set_visible("rect/1", True)


def test_arrows() -> None:
    import numpy as np

    def rotate(theta: float, len: float) -> Tuple[float, float, float]:
        return (
            math.cos(theta) - len * math.sin(theta),
            math.sin(theta) + len * math.cos(theta),
            0.0,
        )

    rerun.log_obb(
        "frame",
        half_size=[20.0, 20.0, 1.0],
        position=[0.0, 0.0, 0.0],
        rotation_q=[0.0, 0.0, 0.0, 0.0],
        timeless=True,
    )

    for t_secs in range(12 * 60 * 60):
        rerun.set_time_seconds("sim_time", t_secs / 60)

        scaled_s = (t_secs % 60) / 60.0
        rerun.log_arrow(
            "seconds_hand",
            origin=[0.0, 0.0, 0.0],
            vector=rotate(2 * pi * scaled_s, 10.0),
            color=(int(255 - (scaled_s * 255)), int(scaled_s * 255), 0, 128),
            width_scale=0.25,
        )
        rerun.log_point("seconds_pt", position=np.array(rotate(2 * pi * scaled_s, 10.0)))

        scaled_m = (t_secs % (3600)) / (3600)
        rerun.log_arrow(
            "minutes_hand",
            origin=[0.0, 0.0, 0.0],
            vector=rotate(2 * pi * scaled_m, 5.0),
            color=(int(255 - (scaled_m * 255)), int(scaled_m * 255), 128, 128),
            width_scale=0.5,
        )
        rerun.log_point("minutes_pt", position=np.array(rotate(2 * pi * scaled_m, 5.0)))

        scaled_h = (t_secs % (43200)) / (43200)
        rerun.log_arrow(
            "hours_hand",
            origin=[0.0, 0.0, 0.0],
            vector=rotate(2 * pi * scaled_h, 2.0),
            color=(int(255 - (scaled_h * 255)), int(scaled_h * 255), 255, 128),
            width_scale=0.8,
        )
        rerun.log_point("hours_pt", position=np.array(rotate(2 * pi * scaled_h, 2.0)))


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--connect",
        dest="connect",
        action="store_true",
        help="Connect to an external viewer",
    )
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")

    # subparsers = parser.add_subparsers(required=True)

    # args_set_visible(subparsers)
    test_arrows()

    args = parser.parse_args()

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    # args.func(args)

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            sleep(100_000)
        except:
            pass

    elif args.save is not None:
        rerun.save(args.save)
    elif not args.connect:
        # Show the logged data inside the Python process:
        rerun.show()


if __name__ == "__main__":
    main()
