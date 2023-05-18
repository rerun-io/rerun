#!/usr/bin/env python3
"""
A collection of many small examples, in one file.

It uses a lot of different aspects of the Rerun API in order to test it.

Example usage:
* Run all demos: `examples/python/api_demo/main.py`
* Run specific demo: `examples/python/api_demo/main.py --demo rects`
"""

import argparse
import logging
import math
import os

import cv2
import numpy as np
import depthai_viewer as viewer
from scipy.spatial.transform import Rotation


def run_segmentation() -> None:
    viewer.set_time_seconds("sim_time", 1)

    # Log an image before we have set up our labels
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    viewer.log_segmentation_image("seg_demo/img", segmentation_img)

    # Log a bunch of classified 2D points
    viewer.log_point("seg_demo/single_point", np.array([64, 64]), class_id=13)
    viewer.log_point("seg_demo/single_point_labeled", np.array([90, 50]), class_id=13, label="labeled point")
    viewer.log_points("seg_demo/several_points0", np.array([[20, 50], [100, 70], [60, 30]]), class_ids=42)
    viewer.log_points(
        "seg_demo/several_points1",
        np.array([[40, 50], [120, 70], [80, 30]]),
        class_ids=np.array([13, 42, 99], dtype=np.uint8),
    )
    viewer.log_points(
        "seg_demo/many points",
        np.array([[100 + (int(i / 5)) * 2, 100 + (i % 5) * 2] for i in range(25)]),
        class_ids=np.array([42], dtype=np.uint8),
    )

    viewer.log_text_entry("logs/seg_demo_log", "default colored rects, default colored points, a single point has a label")

    # Log an initial segmentation map with arbitrary colors
    viewer.set_time_seconds("sim_time", 2)
    viewer.log_annotation_context("seg_demo", [(13, "label1"), (42, "label2"), (99, "label3")], timeless=False)
    viewer.log_text_entry(
        "logs/seg_demo_log",
        "default colored rects, default colored points, " "all points except the bottom right clusters have labels",
    )

    # Log an updated segmentation map with specific colors
    viewer.set_time_seconds("sim_time", 3)
    viewer.log_annotation_context(
        "seg_demo",
        [(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))],
        timeless=False,
    )
    viewer.log_text_entry("logs/seg_demo_log", "points/rects with user specified colors")

    # Log with a mixture of set and unset colors / labels
    viewer.set_time_seconds("sim_time", 4)
    viewer.log_annotation_context(
        "seg_demo",
        [viewer.AnnotationInfo(13, color=(255, 0, 0)), (42, "label2", (0, 255, 0)), viewer.AnnotationInfo(99, label="label3")],
        timeless=False,
    )
    viewer.log_text_entry("logs/seg_demo_log", "label1 disappears and everything with label3 is now default colored again")


def run_2d_lines() -> None:
    import numpy as np

    T = np.linspace(0, 5, 100)
    for n in range(2, len(T)):
        viewer.set_time_seconds("sim_time", T[n])
        t = T[:n]
        x = np.cos(t * 5) * t
        y = np.sin(t * 5) * t
        pts = np.vstack([x, y]).T
        viewer.log_line_strip("2d_lines/spiral", positions=pts)


def run_3d_points() -> None:
    import random

    viewer.set_time_seconds("sim_time", 1)
    viewer.log_point("3d_points/single_point_unlabeled", np.array([10.0, 0.0, 0.0]))
    viewer.log_point("3d_points/single_point_labeled", np.array([0.0, 0.0, 0.0]), label="labeled point")
    viewer.log_points(
        "3d_points/spiral_small",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 + 10.0, i * 4.0 - 5.0] for i in range(9)]),
        labels=[str(i) for i in range(9)],
        radii=np.linspace(0.1, 2.0, num=9),
    )
    viewer.log_points(
        "3d_points/spiral_big",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 - 10.0, i * 0.4 - 5.0] for i in range(100)]),
        labels=[str(i) for i in range(100)],
        colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(100)]),
    )


def raw_mesh() -> None:
    viewer.log_mesh(
        "mesh_demo/triangle",
        positions=[[0, 0, 0], [0, 0.7, 0], [1.0, 0.0, 0]],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    )


def run_rects() -> None:
    import random

    viewer.set_time_seconds("sim_time", 1)

    # Add an image
    img = np.zeros([1024, 1024, 3], dtype="uint8")
    img[:, :] = (128, 128, 128)
    viewer.log_image("rects_demo/img", img)

    # 20 random rectangles
    viewer.set_time_seconds("sim_time", 2)
    rects_xy = np.random.rand(20, 2) * 1024
    rects_wh = np.random.rand(20, 2) * (1024 - rects_xy + 1)
    rects = np.hstack((rects_xy, rects_wh))
    colors = np.array([[random.randrange(255) for _ in range(3)] for _ in range(20)])
    viewer.log_rects("rects_demo/rects", rects, colors=colors, rect_format=viewer.RectFormat.XYWH)

    # Clear the rectangles by logging an empty set
    viewer.set_time_seconds("sim_time", 3)
    viewer.log_rects("rects_demo/rects", [])


def run_text_logs() -> None:
    viewer.log_text_entry("logs", "Text with explicitly set color", color=[255, 215, 0], timeless=True)
    viewer.log_text_entry("logs", "this entry has loglevel TRACE", level=viewer.LogLevel.TRACE)

    logging.getLogger().addHandler(viewer.LoggingHandler("logs/handler"))
    logging.getLogger().setLevel(-1)
    logging.info("This log got added through a `LoggingHandler`")


def run_log_cleared() -> None:
    viewer.set_time_seconds("sim_time", 1)
    viewer.log_rect("null_demo/rect/0", [5, 5, 4, 4], label="Rect1", color=(255, 0, 0))
    viewer.log_rect("null_demo/rect/1", [10, 5, 4, 4], label="Rect2", color=(0, 255, 0))
    viewer.set_time_seconds("sim_time", 2)
    viewer.log_cleared("null_demo/rect/0")
    viewer.set_time_seconds("sim_time", 3)
    viewer.log_cleared("null_demo/rect", recursive=True)
    viewer.set_time_seconds("sim_time", 4)
    viewer.log_rect("null_demo/rect/0", [5, 5, 4, 4])
    viewer.set_time_seconds("sim_time", 5)
    viewer.log_rect("null_demo/rect/1", [10, 5, 4, 4])


def transforms_rigid_3d() -> None:
    viewer.set_time_seconds("sim_time", 0)

    sun_to_planet_distance = 6.0
    planet_to_moon_distance = 3.0
    rotation_speed_planet = 2.0
    rotation_speed_moon = 5.0

    # Planetary motion is typically in the XY plane.
    viewer.log_view_coordinates("transforms3d", up="+Z", timeless=True)
    viewer.log_view_coordinates("transforms3d/sun", up="+Z", timeless=True)
    viewer.log_view_coordinates("transforms3d/sun/planet", up="+Z", timeless=True)
    viewer.log_view_coordinates("transforms3d/sun/planet/moon", up="+Z", timeless=True)

    # All are in the center of their own space:
    viewer.log_point("transforms3d/sun", [0.0, 0.0, 0.0], radius=1.0, color=[255, 200, 10])
    viewer.log_point("transforms3d/sun/planet", [0.0, 0.0, 0.0], radius=0.4, color=[40, 80, 200])
    viewer.log_point("transforms3d/sun/planet/moon", [0.0, 0.0, 0.0], radius=0.15, color=[180, 180, 180])

    # "dust" around the "planet" (and inside, don't care)
    # distribution is quadratically higher in the middle
    radii = np.random.rand(200) * planet_to_moon_distance * 0.5
    angles = np.random.rand(200) * math.tau
    height = np.power(np.random.rand(200), 0.2) * 0.5 - 0.5
    viewer.log_points(
        "transforms3d/sun/planet/dust",
        np.array([np.sin(angles) * radii, np.cos(angles) * radii, height]).transpose(),
        colors=[80, 80, 80],
        radii=0.025,
    )

    # paths where the planet & moon move
    angles = np.arange(0.0, 1.01, 0.01) * math.tau
    circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
    viewer.log_line_strip(
        "transforms3d/sun/planet_path",
        circle * sun_to_planet_distance,
    )
    viewer.log_line_strip(
        "transforms3d/sun/planet/moon_path",
        circle * planet_to_moon_distance,
    )

    # movement via transforms
    for i in range(0, 6 * 120):
        time = i / 120.0
        viewer.set_time_seconds("sim_time", time)
        rotation_q = [0, 0, 0, 1]

        viewer.log_rigid3(
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
        viewer.log_rigid3(
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
    viewer.set_time_seconds("sim_time", 0)
    viewer.log_obb(
        "bbox_demo/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([0.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[0, 255, 0],
        stroke_width=0.01,
        label="box/t0",
    )

    viewer.set_time_seconds("sim_time", 1)
    viewer.log_obb(
        "bbox_demo/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([1.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[255, 255, 0],
        stroke_width=0.02,
        label="box/t1",
    )


def run_extension_component() -> None:
    viewer.set_time_seconds("sim_time", 0)
    # Hack to establish 2d view bounds
    viewer.log_rect("extension_components", [0, 0, 128, 128])

    # Single point
    viewer.log_point("extension_components/point", np.array([64, 64]), color=(255, 0, 0))
    # Separate extension component
    viewer.log_extension_components("extension_components/point", {"confidence": 0.9})

    # Batch points with extension
    # Note: each extension component must either be length 1 (a splat) or the same length as the batch
    viewer.set_time_seconds("sim_time", 1)
    viewer.log_points(
        "extension_components/points",
        np.array([[32, 32], [32, 96], [96, 32], [96, 96]]),
        colors=(0, 255, 0),
        ext={"corner": ["upper left", "lower left", "upper right", "lower right"], "training": True},
    )


def run_image_tensors() -> None:
    # Make sure you use a colorful image with alpha!
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = f"{dir_path}/../../../crates/re_ui/data/logo_dark_mode.png"
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)

    img_rgba = cv2.cvtColor(img_bgra, cv2.COLOR_BGRA2RGBA)
    viewer.log_image("img_rgba", img_rgba)
    img_rgb = cv2.cvtColor(img_rgba, cv2.COLOR_RGBA2RGB)
    viewer.log_image("img_rgb", img_rgb)
    img_gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY)
    viewer.log_image("img_gray", img_gray)

    dtypes = [
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "int8",
        "int16",
        "int32",
        "int64",
        "float16",
        "float32",
        "float64",
    ]

    for dtype in dtypes:
        viewer.log_image(f"img_rgba_{dtype}", img_rgba.astype(dtype))
        viewer.log_image(f"img_rgb_{dtype}", img_rgb.astype(dtype))
        viewer.log_image(f"img_gray_{dtype}", img_gray.astype(dtype))


def main() -> None:
    demos = {
        "2d_lines": run_2d_lines,
        "3d_points": run_3d_points,
        "bbox": run_bounding_box,
        "extension_components": run_extension_component,
        "image_tensors": run_image_tensors,
        "log_cleared": run_log_cleared,
        "raw_mesh": raw_mesh,
        "rects": run_rects,
        "segmentation": run_segmentation,
        "text": run_text_logs,
        "transforms_3d": transforms_rigid_3d,
    }

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--demo", type=str, default="most", help="What demo to run", choices=["most", "all"] + list(demos.keys())
    )

    viewer.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.script_setup(args, "api_demo")

    if args.demo in ["most", "all"]:
        print(f"Running {args.demo} demos…")
        for name, demo in demos.items():
            # Some demos are just a bit… too much
            if args.demo == "most" and name in ["image_tensors"]:
                continue

            logging.info(f"Starting {name}")
            demo()
    else:
        demo = demos[args.demo]
        demo()

    viewer.script_teardown(args)


if __name__ == "__main__":
    main()
