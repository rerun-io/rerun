#!/usr/bin/env python3
"""Minimal examples of Rerun SDK usage.

Example usage:
* Run all demos: `examples/api_demo/main.py`
* Run specific demo: `examples/api_demo/main.py --demo rects`
"""

import argparse
import logging
import math
import os
from pathlib import Path
from time import sleep
from typing import Any, Final

import numpy as np
from rerun.log.annotation import AnnotationInfo
from rerun.log.rects import RectFormat
from rerun.log.text import LoggingHandler, LogLevel
from scipy.spatial.transform import Rotation

import rerun

# from rerun import AnnotationInfo, LoggingHandler, LogLevel, RectFormat


def run_segmentation() -> None:
    rerun.set_time_seconds("sim_time", 1)

    # Log an image before we have set up our labels
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    rerun.log_segmentation_image("seg_demo/img", segmentation_img)

    # Log a bunch of classified 2D points
    rerun.log_point("seg_demo/single_point", np.array([64, 64]), class_id=13)
    rerun.log_point("seg_demo/single_point_labeled", np.array([90, 50]), class_id=13, label="labeled point")
    rerun.log_points("seg_demo/several_points0", np.array([[20, 50], [100, 70], [60, 30]]), class_ids=42)
    rerun.log_points(
        "seg_demo/several_points1",
        np.array([[40, 50], [120, 70], [80, 30]]),
        class_ids=np.array([13, 42, 99], dtype=np.uint8),
    )
    rerun.log_points(
        "seg_demo/many points",
        np.array([[100 + (int(i / 5)) * 2, 100 + (i % 5) * 2] for i in range(25)]),
        class_ids=np.array([42], dtype=np.uint8),
    )

    rerun.log_text_entry("seg_demo_log", "no rects, default colored points, a single point has a label")

    # Log an initial segmentation map with arbitrary colors
    rerun.set_time_seconds("sim_time", 2)
    rerun.log_annotation_context("seg_demo", [(13, "label1"), (42, "label2"), (99, "label3")], timeless=False)
    rerun.log_text_entry(
        "seg_demo_log",
        "default colored rects, default colored points, " "all points except the bottom right clusters have labels",
    )

    # Log an updated segmentation map with specific colors
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_annotation_context(
        "seg_demo",
        [(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))],
        timeless=False,
    )
    rerun.log_text_entry("seg_demo_log", "points/rects with user specified colors")

    # Log with a mixture of set and unset colors / labels
    rerun.set_time_seconds("sim_time", 4)
    rerun.log_annotation_context(
        "seg_demo",
        [AnnotationInfo(13, color=(255, 0, 0)), (42, "label2", (0, 255, 0)), AnnotationInfo(99, label="label3")],
        timeless=False,
    )
    rerun.log_text_entry("seg_demo_log", "label1 disappears and everything with label3 is now default colored again")


def run_points_3d() -> None:
    import random

    rerun.set_time_seconds("sim_time", 1)
    rerun.log_point("3d_points/single_point_unlabeled", np.array([10.0, 0.0, 0.0]))
    rerun.log_point("3d_points/single_point_labeled", np.array([0.0, 0.0, 0.0]), label="labeled point")
    rerun.log_points(
        "3d_points/spiral_small",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 + 10.0, i * 4.0 - 5.0] for i in range(9)]),
        labels=[str(i) for i in range(9)],
        radii=np.linspace(0.1, 2.0, num=9),
    )
    rerun.log_points(
        "3d_points/spiral_big",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 - 10.0, i * 0.4 - 5.0] for i in range(100)]),
        labels=[str(i) for i in range(100)],
        colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(100)]),
    )


def run_rects() -> None:
    import random

    rerun.set_time_seconds("sim_time", 1)

    # Add an image
    img = np.zeros([1024, 1024, 3], dtype="uint8")
    img[:, :] = (128, 128, 128)
    rerun.log_image("rects_demo/img", img)

    # 20 random rectangles
    rerun.set_time_seconds("sim_time", 2)
    rects_xy = np.random.rand(20, 2) * 1024
    rects_wh = np.random.rand(20, 2) * (1024 - rects_xy + 1)
    rects = np.hstack((rects_xy, rects_wh))
    colors = np.array([[random.randrange(255) for _ in range(3)] for _ in range(20)])
    rerun.log_rects("rects_demo/rects", rects, colors=colors, rect_format=RectFormat.XYWH)

    # Clear the rectangles by logging an empty set
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_rects("rects_demo/rects", [])


def run_text_logs() -> None:
    rerun.log_text_entry("logs", "Text with explicitly set color", color=[255, 215, 0], timeless=True)
    rerun.log_text_entry("logs", "this entry has loglevel TRACE", level=LogLevel.TRACE)

    logging.getLogger().addHandler(LoggingHandler("logs/handler"))
    logging.getLogger().setLevel(-1)
    logging.info("This log got added through a `LoggingHandler`")


def run_log_cleared() -> None:
    rerun.set_time_seconds("sim_time", 1)
    rerun.log_rect("null_demo/rect/0", [5, 5, 4, 4], label="Rect1", color=(255, 0, 0))
    rerun.log_rect("null_demo/rect/1", [10, 5, 4, 4], label="Rect2", color=(0, 255, 0))
    rerun.set_time_seconds("sim_time", 2)
    rerun.log_cleared("null_demo/rect/0")
    rerun.set_time_seconds("sim_time", 3)
    rerun.log_cleared("null_demo/rect", recursive=True)
    rerun.set_time_seconds("sim_time", 4)
    rerun.log_rect("null_demo/rect/0", [5, 5, 4, 4])
    rerun.set_time_seconds("sim_time", 5)
    rerun.log_rect("null_demo/rect/1", [10, 5, 4, 4])


def transforms_rigid_3d() -> None:
    rerun.set_time_seconds("sim_time", 0)

    sun_to_planet_distance = 6.0
    planet_to_moon_distance = 3.0
    rotation_speed_planet = 2.0
    rotation_speed_moon = 5.0

    # Planetary motion is typically in the XY plane.
    rerun.log_view_coordinates("transforms3d", up="+Z", timeless=True)
    rerun.log_view_coordinates("transforms3d/sun", up="+Z", timeless=True)
    rerun.log_view_coordinates("transforms3d/sun/planet", up="+Z", timeless=True)
    rerun.log_view_coordinates("transforms3d/sun/planet/moon", up="+Z", timeless=True)

    # All are in the center of their own space:
    rerun.log_point("transforms3d/sun", [0.0, 0.0, 0.0], radius=1.0, color=[255, 200, 10])
    rerun.log_point("transforms3d/sun/planet", [0.0, 0.0, 0.0], radius=0.4, color=[40, 80, 200])
    rerun.log_point("transforms3d/sun/planet/moon", [0.0, 0.0, 0.0], radius=0.15, color=[180, 180, 180])

    # "dust" around the "planet" (and inside, don't care)
    # distribution is quadratically higher in the middle
    radii = np.random.rand(200) * planet_to_moon_distance * 0.5
    angles = np.random.rand(200) * math.tau
    height = np.power(np.random.rand(200), 0.2) * 0.5 - 0.5
    rerun.log_points(
        "transforms3d/sun/planet/dust",
        np.array([np.sin(angles) * radii, np.cos(angles) * radii, height]).transpose(),
        colors=[80, 80, 80],
        radii=0.025,
    )

    # paths where the planet & moon move
    angles = np.arange(0.0, 1.01, 0.01) * math.tau
    circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
    rerun.log_path(
        "transforms3d/sun/planet_path",
        circle * sun_to_planet_distance,
    )
    rerun.log_path(
        "transforms3d/sun/planet/moon_path",
        circle * planet_to_moon_distance,
    )

    # movement via transforms
    for i in range(0, 6 * 120):
        time = i / 120.0
        rerun.set_time_seconds("sim_time", time)
        rotation_q = [0, 0, 0, 1]

        rerun.log_rigid3(
            "transforms3d/sun/planet",
            parent_from_child=(
                [
                    math.sin(time * rotation_speed_planet) * sun_to_planet_distance,
                    math.cos(time * rotation_speed_planet) * sun_to_planet_distance,
                    0.0,
                ],
                Rotation.from_euler("x", 20, degrees=True).as_quat(),
            ),
        )
        rerun.log_rigid3(
            "transforms3d/sun/planet/moon",
            child_from_parent=(
                [
                    math.cos(time * rotation_speed_moon) * planet_to_moon_distance,
                    math.sin(time * rotation_speed_moon) * planet_to_moon_distance,
                    0.0,
                ],
                rotation_q,
            ),
        )


def run_bounding_box() -> None:
    rerun.set_time_seconds("sim_time", 0)
    rerun.log_obb(
        "bbox_demo/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([0.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[0, 255, 0],
        stroke_width=0.01,
        label="box/t0",
    )

    rerun.set_time_seconds("sim_time", 1)
    rerun.log_obb(
        "bbox_demo/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([1.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[255, 255, 0],
        stroke_width=0.02,
        label="box/t1",
    )


def main() -> None:
    demos = {
        "3d_points": run_points_3d,
        "log_cleared": run_log_cleared,
        "rects": run_rects,
        "segmentation": run_segmentation,
        "text": run_text_logs,
        "transforms_3d": transforms_rigid_3d,
        "bbox": run_bounding_box,
    }

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
