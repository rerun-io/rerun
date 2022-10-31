#!/usr/bin/env python3
"""Minimal examples of Rerun SDK usage.

Example usage:
* Run all demos: `examples/api_demo/main.py`
* Run specific demo: `examples/api_demo/main.py --demo set_visible`
"""

import argparse
import os
from pathlib import Path
from time import sleep
from typing import Any, Final

import numpy as np
import rerun_sdk as rerun
from rerun_sdk import ClassDescription, RectFormat


def run_misc() -> None:
    CAMERA_GLB: Final = Path(os.path.dirname(__file__)).joinpath("../../crates/re_viewer/data/camera.glb")
    mesh_data = CAMERA_GLB.read_bytes()

    # Optional affine transformation matrix to apply (in this case: scale it up by a factor x2)
    transform = np.array(
        [
            [2, 0, 0, 0],
            [0, 2, 0, 0],
            [0, 0, 2, 0],
        ]
    )
    rerun.log_mesh_file("world/example_mesh", rerun.MeshFormat.GLB, mesh_data, transform=transform)

    rerun.log_path("world/a_box", np.array([[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 0, 0]]))


def run_segmentation() -> None:
    rerun.set_time_seconds("sim_time", 1)

    # Log an image before we have set up our labels
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    rerun.log_segmentation_image("img", segmentation_img, "img/labels")

    # Log an initial segmentation map with arbitrary colors
    rerun.set_time_seconds("sim_time", 2)
    rerun.log_class_descriptions("img/labels", [(13, "label1"), (42, "label2"), (99, "label3")])

    # Log an updated segmentation map with specific colors
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_class_descriptions(
        "img/labels", [(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))]
    )

    # Log with a mixture of set and unset colors / labels
    rerun.set_time_seconds("sim_time", 4)
    rerun.log_class_descriptions(
        "img/labels",
        [ClassDescription(13, color=(255, 0, 0)), (42, "label2", (0, 255, 0)), ClassDescription(99, label="label3")],
    )


def run_set_visible() -> None:
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


def run_rects() -> None:
    import random

    rerun.set_time_seconds("sim_time", 1)

    # Add an image
    img = np.zeros([1024, 1024, 3], dtype="uint8")
    img[:, :] = (128, 128, 128)
    rerun.log_image("img", img)

    # 20 random rectangles
    rerun.set_time_seconds("sim_time", 2)
    rects_x = [random.randrange(0, 1024) for _ in range(20)]
    rects_y = [random.randrange(0, 1024) for _ in range(20)]
    rects_w = [random.randrange(0, 1024 - x + 1) for x in rects_x]
    rects_h = [random.randrange(0, 1024 - y + 1) for y in rects_y]
    rects = [(x, y, w, h) for x, y, w, h in zip(rects_x, rects_y, rects_w, rects_h)]
    colors = np.array([[random.randrange(255) for _ in range(3)] for _ in range(20)])
    rerun.log_rects("img/rects", rects, colors=colors, rect_format=RectFormat.XYWH)

    # Clear the rectangles by logging an empty set
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_rects("img/rects", [])


def main() -> None:
    demos = {"misc": run_misc, "segmentation": run_segmentation, "set_visible": run_set_visible, "rects": run_rects}

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--demo", type=str, default="all", help="What demo to run", choices=["all"] + list(demos.keys())
    )

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

    args = parser.parse_args()

    rerun.init("api_demo")

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    if args.demo == "all":
        print("Running all demos…")
        for name, demo in demos.items():
            demo()
    else:
        demo = demos[args.demo]
        demo()

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
