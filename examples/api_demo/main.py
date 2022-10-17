#!/usr/bin/env python3
"""Minimal examples of Rerun SDK usage.

set_visible:
Uses `rerun.set_visible` to toggle the visibility of some rects
"""

import argparse
from time import sleep
from typing import Any

import numpy as np
import rerun_sdk as rerun
from rerun_sdk import Mapping


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


def args_segmentation(subparsers: Any) -> None:
    segmentation_parser = subparsers.add_parser("segmentation")
    segmentation_parser.set_defaults(func=run_segmentation)


def run_segmentation(args: argparse.Namespace) -> None:
    rerun.set_time_seconds("sim_time", 1)

    # Log an image
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    rerun.log_image("img", segmentation_img, legend="seg")

    # Log an initial segmentation map with arbitrary colors
    rerun.log_segmentation_map("seg", {13: "label1", 42: "label2", 99: "label3"})

    # Log an updated segmentation map with specific colors
    rerun.set_time_seconds("sim_time", 2)
    rerun.log_segmentation_map(
        "seg", {13: ("label1", (255, 0, 0)), 42: ("label2", (0, 255, 0)), 99: ("label3", (0, 0, 255))}
    )

    # Log with a mixture of set and unset colors / labels
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_segmentation_map("seg", {13: Mapping(color=(255, 0, 0)), 42: ("label2", (0, 255, 0)), 99: "label3"})


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

    subparsers = parser.add_subparsers(required=True)

    args_set_visible(subparsers)
    args_segmentation(subparsers)

    args = parser.parse_args()

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    args.func(args)

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
