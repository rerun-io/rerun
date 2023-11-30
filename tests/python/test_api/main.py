#!/usr/bin/env python3
"""
A collection of many small examples, in one file.

It uses a lot of different aspects of the Rerun API in order to test it.

Example usage:
* Run all tests: `examples/python/test_api/main.py`
* Run specific test: `examples/python/test_api/main.py --test rects`
"""
from __future__ import annotations

import argparse
import logging
import math
import os
import random
import threading
from typing import Callable

import cv2
import numpy as np
import rerun as rr  # pip install rerun-sdk


def run_segmentation() -> None:
    rr.set_time_seconds("sim_time", 1)

    # Log an image before we have set up our labels
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    rr.log("seg_test/img", rr.SegmentationImage(segmentation_img))

    # Log a bunch of classified 2D points
    rr.log("seg_test/single_point", rr.Points2D([64, 64], class_ids=13))
    rr.log("seg_test/single_point_labeled", rr.Points2D([90, 50], class_ids=13, labels="labeled point"))
    rr.log("seg_test/several_points0", rr.Points2D([[20, 50], [100, 70], [60, 30]], class_ids=42))
    rr.log(
        "seg_test/several_points1",
        rr.Points2D([[40, 50], [120, 70], [80, 30]], class_ids=np.array([13, 42, 99], dtype=np.uint8)),
    )
    rr.log(
        "seg_test/many_points",
        rr.Points2D(
            [[100 + (int(i / 5)) * 2, 100 + (i % 5) * 2] for i in range(25)],
            class_ids=np.array([42], dtype=np.uint8),
        ),
    )

    rr.log("logs/seg_test_log", rr.TextLog("default colored rects, default colored points, a single point has a label"))

    # Log an initial segmentation map with arbitrary colors
    rr.set_time_seconds("sim_time", 2)

    rr.log("seg_test", rr.AnnotationContext([(13, "label1"), (42, "label2"), (99, "label3")]), timeless=False)

    rr.log(
        "logs/seg_test_log",
        rr.TextLog(
            "default colored rects, default colored points, " "all points except the bottom right clusters have labels",
        ),
    )

    # Log an updated segmentation map with specific colors
    rr.set_time_seconds("sim_time", 3)
    rr.log(
        "seg_test",
        rr.AnnotationContext([(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))]),
        timeless=False,
    )
    rr.log("logs/seg_test_log", rr.TextLog("points/rects with user specified colors"))

    # Log with a mixture of set and unset colors / labels
    rr.set_time_seconds("sim_time", 4)

    rr.log(
        "seg_test",
        rr.AnnotationContext(
            [
                rr.AnnotationInfo(13, color=(255, 0, 0)),
                (42, "label2", (0, 255, 0)),
                rr.AnnotationInfo(99, label="label3"),
            ]
        ),
        timeless=False,
    )
    rr.log("logs/seg_test_log", rr.TextLog("label1 disappears and everything with label3 is now default colored again"))


def small_image() -> None:
    img = [
        [[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        [[0, 0, 255], [255, 0, 0], [0, 255, 0]],
    ]
    rr.log("small_image", rr.Image(img))


def run_2d_layering() -> None:
    rr.set_time_seconds("sim_time", 1)

    # Large gray background.
    img = np.full((512, 512), 64, dtype="uint8")
    rr.log("2d_layering/background", rr.Image(img, draw_order=0.0))

    # Smaller gradient in the middle.
    img = np.zeros((256, 256, 3), dtype="uint8")
    img[:, :, 0] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = img[:, :, 1].transpose()
    rr.log("2d_layering/middle_gradient", rr.Image(img, draw_order=1.0))

    # Slightly smaller blue in the middle, on the same layer as the previous.
    img = np.full((192, 192, 3), (0, 0, 255), dtype="uint8")
    rr.log("2d_layering/middle_blue", rr.Image(img, draw_order=1.0))

    # Small white on top.
    img = np.full((128, 128), 255, dtype="uint8")
    rr.log("2d_layering/top", rr.Image(img, draw_order=2.0))

    # Rectangle in between the top and the middle.
    rr.log(
        "2d_layering/rect_between_top_and_middle",
        rr.Boxes2D(array=[64, 64, 256, 256], draw_order=1.5, array_format=rr.Box2DFormat.XYWH),
    )

    # Lines behind the rectangle.
    rr.log(
        "2d_layering/lines_behind_rect",
        rr.LineStrips2D([(i * 20, i % 2 * 100 + 100) for i in range(20)], draw_order=1.25),
    )

    # And some points in front of the rectangle.
    rr.log(
        "2d_layering/points_between_top_and_middle",
        rr.Points2D(
            [(32.0 + int(i / 16) * 16.0, 64.0 + (i % 16) * 16.0) for i in range(16 * 16)],
            draw_order=1.51,
        ),
    )


def transforms() -> None:
    rr.log("transforms", rr.ViewCoordinates.RIGHT_HAND_Y_UP, timeless=True)

    # Log a disconnected space (this doesn't do anything here, but can be used to force a new space)
    rr.log("transforms/disconnected", rr.DisconnectedSpace())

    # Log scale along the x axis only.
    rr.log("transforms/x_scaled", rr.Transform3D(scale=(3, 1, 1)))

    # Log a rotation around the z axis.
    rr.log(
        "transforms/z_rotated_object",
        rr.Transform3D(rotation=rr.RotationAxisAngle(axis=(1, 0, 0), degrees=45)),
    )

    # Log a transform from parent to child with a translation and skew along y and x.
    rr.log(
        "transforms/child_from_parent_translation",
        rr.Transform3D(translation=(-1, 0, 0), from_parent=True),
    )

    # Log translation only.
    rr.log("transforms/translation", rr.Transform3D(translation=(2, 0, 0)))
    rr.log("transforms/translation2", rr.Transform3D(translation=(3, 0, 0)))

    # Log uniform scale followed by translation along the Y-axis.
    rr.log(
        "transforms/scaled_and_translated_object",
        rr.Transform3D(translation=[0, 0, 1], scale=3),
    )

    # Log translation + rotation, also called a rigid transform.
    rr.log(
        "transforms/rigid3",
        rr.Transform3D(translation=[1, 0, 1], rotation=rr.RotationAxisAngle(axis=(0, 1, 0), radians=1.57)),
    )

    # Log translation, rotation & scale all at once.
    rr.log(
        "transforms/transformed",
        rr.Transform3D(
            translation=[2, 0, 1],
            rotation=rr.RotationAxisAngle(axis=(0, 0, 1), degrees=20),
            scale=2,
        ),
    )

    # Log a transform with translation and shear along x.
    rr.log(
        "transforms/shear",
        rr.Transform3D(translation=(3, 0, 1), mat3x3=np.array([[1, 1, 0], [0, 1, 0], [0, 0, 1]])),
    )


def run_2d_lines() -> None:
    import numpy as np

    T = np.linspace(0, 5, 100)
    for n in range(2, len(T)):
        rr.set_time_seconds("sim_time", T[n])
        t = T[:n]
        x = np.cos(t * 5) * t
        y = np.sin(t * 5) * t
        pts = np.vstack([x, y]).T
        rr.log("2d_lines/spiral", rr.LineStrips2D(strips=pts))


def run_3d_points() -> None:
    rr.set_time_seconds("sim_time", 1)
    rr.log("3d_points/single_point_unlabeled", rr.Points3D(np.array([10.0, 0.0, 0.0])))
    rr.log("3d_points/single_point_labeled", rr.Points3D(np.array([0.0, 0.0, 0.0]), labels="labeled point"))
    rr.log(
        "3d_points/spiral_small",
        rr.Points3D(
            np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 + 10.0, i * 4.0 - 5.0] for i in range(9)]),
            labels=[str(i) for i in range(9)],
            radii=np.linspace(0.1, 2.0, num=9),
        ),
    )
    rr.log(
        "3d_points/spiral_big",
        rr.Points3D(
            np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 - 10.0, i * 0.4 - 5.0] for i in range(100)]),
            labels=[str(i) for i in range(100)],
            colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(100)]),
        ),
    )


def raw_mesh() -> None:
    rr.log(
        "mesh_test/triangle",
        rr.Mesh3D(
            vertex_positions=[[0, 0, 0], [0, 0.7, 0], [1.0, 0.0, 0]],
            vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        ),
    )


def run_rects() -> None:
    rr.set_time_seconds("sim_time", 1)

    # Add an image
    img = np.zeros([1024, 1024, 3], dtype="uint8")
    img[:, :] = (128, 128, 128)
    rr.log("rects_test/img", rr.Image(img))

    # 20 random rectangles
    rr.set_time_seconds("sim_time", 2)
    rects_xy = np.random.rand(20, 2) * 1024
    rects_wh = np.random.rand(20, 2) * (1024 - rects_xy + 1)
    rects = np.hstack((rects_xy, rects_wh))
    colors = np.array([[random.randrange(255) for _ in range(3)] for _ in range(20)])
    rr.log("rects_test/rects", rr.Boxes2D(array=rects, colors=colors, array_format=rr.Box2DFormat.XYWH))

    # Clear the rectangles by logging an empty set
    rr.set_time_seconds("sim_time", 3)
    rr.log("rects_test/rects", rr.Boxes2D(sizes=[]))


def run_text_logs() -> None:
    rr.log("logs", rr.TextLog("Text with explicitly set color", color=[255, 215, 0]), timeless=True)
    rr.log("logs", rr.TextLog("this entry has loglevel TRACE", level="TRACE"))

    logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
    logging.getLogger().setLevel(-1)
    logging.info("This log got added through a `LoggingHandler`")


def run_log_cleared() -> None:
    rr.set_time_seconds("sim_time", 1)
    rr.log(
        "null_test/rect/0",
        rr.Boxes2D(array=[5, 5, 4, 4], array_format=rr.Box2DFormat.XYWH, labels="Rect1", colors=(255, 0, 0)),
    )
    rr.log(
        "null_test/rect/1",
        rr.Boxes2D(array=[10, 5, 4, 4], array_format=rr.Box2DFormat.XYWH, labels="Rect2", colors=(0, 255, 0)),
    )
    rr.set_time_seconds("sim_time", 2)
    rr.log("null_test/rect/0", rr.Clear(recursive=False))
    rr.set_time_seconds("sim_time", 3)
    rr.log("null_test/rect", rr.Clear(recursive=True))
    rr.set_time_seconds("sim_time", 4)
    rr.log("null_test/rect/0", rr.Boxes2D(array=[5, 5, 4, 4], array_format=rr.Box2DFormat.XYWH))
    rr.set_time_seconds("sim_time", 5)
    rr.log("null_test/rect/1", rr.Boxes2D(array=[10, 5, 4, 4], array_format=rr.Box2DFormat.XYWH))


def transforms_rigid_3d() -> None:
    rr.set_time_seconds("sim_time", 0)

    sun_to_planet_distance = 6.0
    planet_to_moon_distance = 3.0
    rotation_speed_planet = 2.0
    rotation_speed_moon = 5.0

    # Planetary motion is typically in the XY plane.
    rr.log("transforms3d", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)
    rr.log("transforms3d/sun", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)
    rr.log("transforms3d/sun/planet", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)
    rr.log("transforms3d/sun/planet/moon", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)

    # All are in the center of their own space:
    rr.log("transforms3d/sun", rr.Points3D([0.0, 0.0, 0.0], radii=1.0, colors=[255, 200, 10]))
    rr.log("transforms3d/sun/planet", rr.Points3D([0.0, 0.0, 0.0], radii=0.4, colors=[40, 80, 200]))
    rr.log("transforms3d/sun/planet/moon", rr.Points3D([0.0, 0.0, 0.0], radii=0.15, colors=[180, 180, 180]))

    # "dust" around the "planet" (and inside, don't care)
    # distribution is quadratically higher in the middle
    radii = np.random.rand(200) * planet_to_moon_distance * 0.5
    angles = np.random.rand(200) * math.tau
    height = np.power(np.random.rand(200), 0.2) * 0.5 - 0.5
    rr.log(
        "transforms3d/sun/planet/dust",
        rr.Points3D(
            np.array([np.sin(angles) * radii, np.cos(angles) * radii, height]).transpose(),
            colors=[80, 80, 80],
            radii=0.025,
        ),
    )

    # paths where the planet & moon move
    angles = np.arange(0.0, 1.01, 0.01) * math.tau
    circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
    rr.log(
        "transforms3d/sun/planet_path",
        rr.LineStrips3D(
            circle * sun_to_planet_distance,
        ),
    )
    rr.log(
        "transforms3d/sun/planet/moon_path",
        rr.LineStrips3D(
            circle * planet_to_moon_distance,
        ),
    )

    # movement via transforms
    for i in range(0, 6 * 120):
        time = i / 120.0
        rr.set_time_seconds("sim_time", time)

        rr.log(
            "transforms3d/sun/planet",
            rr.Transform3D(
                translation=[
                    math.sin(time * rotation_speed_planet) * sun_to_planet_distance,
                    math.cos(time * rotation_speed_planet) * sun_to_planet_distance,
                    0.0,
                ],
                rotation=rr.RotationAxisAngle(axis=(1, 0, 0), degrees=20),
            ),
        )
        rr.log(
            "transforms3d/sun/planet/moon",
            rr.Transform3D(
                translation=[
                    math.cos(time * rotation_speed_moon) * planet_to_moon_distance,
                    math.sin(time * rotation_speed_moon) * planet_to_moon_distance,
                    0.0,
                ],
                from_parent=True,
            ),
        )


def run_bounding_box() -> None:
    rr.set_time_seconds("sim_time", 0)
    rr.log(
        "bbox_test/bbox",
        rr.Boxes3D(
            half_sizes=[1.0, 0.5, 0.25],
            rotations=rr.Quaternion(xyzw=[0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
            colors=[0, 255, 0],
            radii=0.01,
            labels="box/t0",
        ),
    )

    rr.set_time_seconds("sim_time", 1)
    rr.log(
        "bbox_test/bbox",
        rr.Boxes3D(
            centers=np.array([1.0, 0.0, 0.0]),
            half_sizes=[1.0, 0.5, 0.25],
            rotations=rr.Quaternion(xyzw=[0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
            colors=[255, 255, 0],
            radii=0.02,
            labels="box/t1",
        ),
    )


def run_extension_component() -> None:
    rr.set_time_seconds("sim_time", 0)
    # Hack to establish 2d view bounds
    rr.log("extension_components", rr.Boxes2D(array=[0, 0, 128, 128], array_format=rr.Box2DFormat.XYWH))

    # Single point
    rr.log("extension_components/point", rr.Points2D(np.array([64, 64]), colors=(255, 0, 0)))
    # Separate extension component
    rr.log("extension_components/point", rr.AnyValues(confidence=0.9))

    # Batch points with extension
    # Note: each extension component must either be length 1 (a splat) or the same length as the batch
    rr.set_time_seconds("sim_time", 1)
    rr.log(
        "extension_components/points",
        rr.Points2D(np.array([[32, 32], [32, 96], [96, 32], [96, 96]]), colors=(0, 255, 0)),
        rr.AnyValues(corner=["upper left", "lower left", "upper right", "lower right"], training=True),
    )


def run_gradient_image() -> None:
    rr.log(
        "gradient_explain",
        rr.TextLog("gradients should look the same, and external color picket should show 128 in the middle"),
    )

    x, _ = np.meshgrid(np.arange(0, 256), np.arange(0, 64))
    im = x.astype(np.uint8)
    rr.log("gradient_u8", rr.Image(im))

    x, _ = np.meshgrid(np.arange(0, 255), np.arange(0, 64))
    im = (x / 255.0).astype(np.float32)
    rr.log("gradient_f32", rr.Image(im))

    x, _ = np.meshgrid(np.arange(0, 256), np.arange(0, 64))
    im = (x * 4).astype(np.uint16)
    rr.log("gradient_u16_0_1020", rr.Image(im))


def run_image_tensors() -> None:
    # Make sure you use a colorful image with alpha!
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = f"{dir_path}/../../../crates/re_ui/data/logo_dark_mode.png"
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)

    img_rgba = cv2.cvtColor(img_bgra, cv2.COLOR_BGRA2RGBA)
    rr.log("img_rgba", rr.Image(img_rgba))
    img_rgb = cv2.cvtColor(img_rgba, cv2.COLOR_RGBA2RGB)
    rr.log("img_rgb", rr.Image(img_rgb))
    img_gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY)
    rr.log("img_gray", rr.Image(img_gray))

for dtype in dtypes:
        rr.log(f"img_rgba_{dtype}", rr.Image(img_rgba.astype(dtype)))
        rr.log(f"img_rgb_{dtype}", rr.Image(img_rgb.astype(dtype)))
        rr.log(f"img_gray_{dtype}", rr.Image(img_gray.astype(dtype)))


def spawn_test(test: Callable[[], None], rec: rr.RecordingStream) -> None:
    with rec:
        test()


def main() -> None:
    tests = {
        "2d_layering": run_2d_layering,
        "2d_lines": run_2d_lines,
        "3d_points": run_3d_points,
        "bbox": run_bounding_box,
        "extension_components": run_extension_component,
        "gradient_image": run_gradient_image,
        "image_tensors": run_image_tensors,
        "log_cleared": run_log_cleared,
        "raw_mesh": raw_mesh,
        "rects": run_rects,
        "segmentation": run_segmentation,
        "small_image": small_image,
        "text": run_text_logs,
        "transforms": transforms,
        "transforms_rigid_3d": transforms_rigid_3d,
    }

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--test", type=str, default="most", help="What test to run", choices=["most", "all"] + list(tests.keys())
    )
    parser.add_argument(
        "--multithread",
        dest="multithread",
        action="store_true",
        help="If specified, each test will be run from its own python thread",
    )
    parser.add_argument(
        "--split-recordings",
        dest="split_recordings",
        action="store_true",
        help="If specified, each test will be its own recording",
    )

    rr.script_add_args(parser)
    args = parser.parse_args()

    if not args.split_recordings:
        rec = rr.script_setup(args, f"rerun_example_test_api_{args.test}")

    if args.test in ["most", "all"]:
        print(f"Running {args.test} tests…")

        threads = []
        for name, test in tests.items():
            # Some tests are just a bit… too much
            if args.test == "most" and name in ["image_tensors", "transforms"]:
                continue

            if args.split_recordings:
                rec = rr.script_setup(args, f"rerun_example_test_api/{name}")

            if args.multithread:
                t = threading.Thread(
                    target=spawn_test,
                    args=(test, rec),
                )
                t.start()
                threads.append(t)
            else:
                logging.info(f"Starting {name}")
                with rec:
                    test()

        for t in threads:
            t.join()
    else:
        if args.split_recordings:
            with rr.script_setup(args, f"rerun_example_test_api/{args.test}"):
                tests[args.test]()
        else:
            tests[args.test]()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
