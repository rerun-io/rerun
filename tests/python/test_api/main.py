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


def run_segmentation(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 1)

    # Log an image before we have set up our labels
    segmentation_img = np.zeros([128, 128], dtype="uint8")
    segmentation_img[10:20, 30:50] = 13
    segmentation_img[80:100, 60:80] = 42
    segmentation_img[20:50, 90:110] = 99
    rr.log_segmentation_image("seg_test/img", segmentation_img)

    # Log a bunch of classified 2D points
    if experimental_api:
        import rerun.experimental as rr2

        # Note: this uses the new, WIP object-oriented API
        rr2.log("seg_test/single_point", rr2.Points2D([64, 64], class_ids=13))
        rr2.log("seg_test/single_point_labeled", rr2.Points2D([90, 50], class_ids=13, labels="labeled point"))
        rr2.log("seg_test/several_points0", rr2.Points2D([[20, 50], [100, 70], [60, 30]], class_ids=42))
        rr2.log(
            "seg_test/several_points1",
            rr2.Points2D([[40, 50], [120, 70], [80, 30]], class_ids=np.array([13, 42, 99], dtype=np.uint8)),
        )
        rr2.log(
            "seg_test/many points",
            rr2.Points2D(
                [[100 + (int(i / 5)) * 2, 100 + (i % 5) * 2] for i in range(25)],
                class_ids=np.array([42], dtype=np.uint8),
            ),
        )
    else:
        rr.log_point("seg_test/single_point", np.array([64, 64]), class_id=13)
        rr.log_point("seg_test/single_point_labeled", np.array([90, 50]), class_id=13, label="labeled point")
        rr.log_points("seg_test/several_points0", np.array([[20, 50], [100, 70], [60, 30]]), class_ids=42)
        rr.log_points(
            "seg_test/several_points1",
            np.array([[40, 50], [120, 70], [80, 30]]),
            class_ids=np.array([13, 42, 99], dtype=np.uint8),
        )
        rr.log_points(
            "seg_test/many points",
            np.array([[100 + (int(i / 5)) * 2, 100 + (i % 5) * 2] for i in range(25)]),
            class_ids=np.array([42], dtype=np.uint8),
        )

    rr.log_text_entry("logs/seg_test_log", "default colored rects, default colored points, a single point has a label")

    # Log an initial segmentation map with arbitrary colors
    rr.set_time_seconds("sim_time", 2)

    if experimental_api:
        rr2.log("seg_test", rr2.AnnotationContext([(13, "label1"), (42, "label2"), (99, "label3")]), timeless=False)
    else:
        rr.log_annotation_context("seg_test", [(13, "label1"), (42, "label2"), (99, "label3")], timeless=False)

    rr.log_text_entry(
        "logs/seg_test_log",
        "default colored rects, default colored points, " "all points except the bottom right clusters have labels",
    )

    # Log an updated segmentation map with specific colors
    rr.set_time_seconds("sim_time", 3)
    if experimental_api:
        rr2.log(
            "seg_test",
            rr2.AnnotationContext(
                [(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))]
            ),
            timeless=False,
        )
    else:
        rr.log_annotation_context(
            "seg_test",
            [(13, "label1", (255, 0, 0)), (42, "label2", (0, 255, 0)), (99, "label3", (0, 0, 255))],
            timeless=False,
        )
    rr.log_text_entry("logs/seg_test_log", "points/rects with user specified colors")

    # Log with a mixture of set and unset colors / labels
    rr.set_time_seconds("sim_time", 4)
    if experimental_api:
        rr2.log(
            "seg_test",
            rr2.AnnotationContext(
                [
                    rr2.dt.AnnotationInfo(13, color=(255, 0, 0)),
                    (42, "label2", (0, 255, 0)),
                    rr2.dt.AnnotationInfo(99, label="label3"),
                ]
            ),
            timeless=False,
        )
    else:
        rr.log_annotation_context(
            "seg_test",
            [
                rr.AnnotationInfo(13, color=(255, 0, 0)),
                (42, "label2", (0, 255, 0)),
                rr.AnnotationInfo(99, label="label3"),
            ],
            timeless=False,
        )
    rr.log_text_entry("logs/seg_test_log", "label1 disappears and everything with label3 is now default colored again")


def small_image(experimental_api: bool) -> None:
    img = [
        [[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        [[0, 0, 255], [255, 0, 0], [0, 255, 0]],
    ]
    rr.log_image("small_image", img)


def run_2d_layering(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 1)

    # Large gray background.
    img = np.full((512, 512), 64, dtype="uint8")
    rr.log_image("2d_layering/background", img, draw_order=0.0)

    # Smaller gradient in the middle.
    img = np.zeros((256, 256, 3), dtype="uint8")
    img[:, :, 0] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = img[:, :, 1].transpose()
    rr.log_image("2d_layering/middle_gradient", img, draw_order=1.0)

    # Slightly smaller blue in the middle, on the same layer as the previous.
    img = np.full((192, 192, 3), (0, 0, 255), dtype="uint8")
    rr.log_image("2d_layering/middle_blue", img, draw_order=1.0)

    # Small white on top.
    img = np.full((128, 128), 255, dtype="uint8")
    rr.log_image("2d_layering/top", img, draw_order=2.0)

    # Rectangle in between the top and the middle.
    rr.log_rect("2d_layering/rect_between_top_and_middle", (64, 64, 256, 256), draw_order=1.5)

    # Lines behind the rectangle.
    rr.log_line_strip(
        "2d_layering/lines_behind_rect", [(i * 20, i % 2 * 100 + 100) for i in range(20)], draw_order=1.25
    )

    # And some points in front of the rectangle.
    rr.log_points(
        "2d_layering/points_between_top_and_middle",
        [(32.0 + int(i / 16) * 16.0, 64.0 + (i % 16) * 16.0) for i in range(16 * 16)],
        draw_order=1.51,
    )


def transforms(experimental_api: bool) -> None:
    rr.log_view_coordinates("transforms", up="+Y")

    # Log a disconnected space (this doesn't do anything here, but can be used to force a new space)
    rr.log_disconnected_space("transforms/disconnected")

    if experimental_api:
        import rerun.experimental as rr2
        from rerun.experimental import dt as rrd

        # Log scale along the x axis only.
        rr2.log("transforms/x_scaled", rrd.TranslationRotationScale3D(scale=(3, 1, 1)))

        # Log a rotation around the z axis.
        rr2.log(
            "transforms/z_rotated_object",
            rrd.TranslationRotationScale3D(rotation=rrd.RotationAxisAngle(axis=(1, 0, 0), angle=rrd.Angle(deg=45))),
        )

        # Log a transform from parent to child with a translation and skew along y and x.
        rr2.log(
            "transforms/child_from_parent_translation",
            rrd.TranslationRotationScale3D(translation=(-1, 0, 0), from_parent=True),
        )

        # Log translation only.
        rr2.log("transforms/translation", rrd.TranslationRotationScale3D(translation=(2, 0, 0)))
        rr2.log("transforms/translation2", rrd.TranslationAndMat3x3(translation=(3, 0, 0)))

        # Log uniform scale followed by translation along the Y-axis.
        rr2.log(
            "transforms/scaled_and_translated_object",
            rrd.TranslationRotationScale3D(translation=[0, 0, 1], scale=3),
        )

        # Log translation + rotation, also called a rigid transform.
        rr2.log(
            "transforms/rigid3",
            rrd.TranslationRotationScale3D(
                translation=[1, 0, 1], rotation=rrd.RotationAxisAngle(axis=(0, 1, 0), angle=rrd.Angle(rad=1.57))
            ),
        )

        # Log translation, rotation & scale all at once.
        rr2.log(
            "transforms/transformed",
            rrd.TranslationRotationScale3D(
                translation=[2, 0, 1],
                rotation=rrd.RotationAxisAngle(axis=(0, 0, 1), angle=rrd.Angle(deg=20)),
                scale=2,
            ),
        )

        # Log a transform with translation and shear along x.
        rr2.log(
            "transforms/shear",
            rrd.TranslationAndMat3x3(translation=(3, 0, 1), matrix=np.array([[1, 1, 0], [0, 1, 0], [0, 0, 1]])),
        )
    else:
        # Log scale along the x axis only.
        rr.log_transform3d("transforms/x_scaled", rr.Scale3D((3, 1, 1)))

        # Log a rotation around the z axis.
        rr.log_transform3d("transforms/z_rotated_object", rr.RotationAxisAngle((1, 0, 0), degrees=45))

        # Log a transform from parent to child with a translation and skew along y and x.
        rr.log_transform3d(
            "transforms/child_from_parent_translation",
            rr.Translation3D((-1, 0, 0)),
            from_parent=True,
        )

        # Log translation only.
        rr.log_transform3d("transforms/translation", rr.Translation3D((2, 0, 0)))
        rr.log_transform3d("transforms/translation2", rr.TranslationAndMat3((3, 0, 0)))

        # Log uniform scale followed by translation along the Y-axis.
        rr.log_transform3d("transforms/scaled_and_translated_object", rr.TranslationRotationScale3D([0, 0, 1], scale=3))

        # Log translation + rotation, also called a rigid transform.
        rr.log_transform3d("transforms/rigid3", rr.Rigid3D([1, 0, 1], rr.RotationAxisAngle((0, 1, 0), radians=1.57)))

        # Log translation, rotation & scale all at once.
        rr.log_transform3d(
            "transforms/transformed",
            rr.TranslationRotationScale3D(
                translation=[2, 0, 1],
                rotation=rr.RotationAxisAngle((0, 0, 1), degrees=20),
                scale=2,
            ),
        )

        # Log a transform with translation and shear along x.
        rr.log_transform3d(
            "transforms/shear",
            rr.TranslationAndMat3((3, 0, 1), np.array([[1, 1, 0], [0, 1, 0], [0, 0, 1]])),
        )


def run_2d_lines(experimental_api: bool) -> None:
    import numpy as np

    T = np.linspace(0, 5, 100)
    for n in range(2, len(T)):
        rr.set_time_seconds("sim_time", T[n])
        t = T[:n]
        x = np.cos(t * 5) * t
        y = np.sin(t * 5) * t
        pts = np.vstack([x, y]).T
        rr.log_line_strip("2d_lines/spiral", positions=pts)


def run_3d_points(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 1)
    rr.log_point("3d_points/single_point_unlabeled", np.array([10.0, 0.0, 0.0]))
    rr.log_point("3d_points/single_point_labeled", np.array([0.0, 0.0, 0.0]), label="labeled point")
    rr.log_points(
        "3d_points/spiral_small",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 + 10.0, i * 4.0 - 5.0] for i in range(9)]),
        labels=[str(i) for i in range(9)],
        radii=np.linspace(0.1, 2.0, num=9),
    )
    rr.log_points(
        "3d_points/spiral_big",
        np.array([[math.sin(i * 0.2) * 5, math.cos(i * 0.2) * 5 - 10.0, i * 0.4 - 5.0] for i in range(100)]),
        labels=[str(i) for i in range(100)],
        colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(100)]),
    )


def raw_mesh(experimental_api: bool) -> None:
    rr.log_mesh(
        "mesh_test/triangle",
        positions=[[0, 0, 0], [0, 0.7, 0], [1.0, 0.0, 0]],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    )


def run_rects(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 1)

    # Add an image
    img = np.zeros([1024, 1024, 3], dtype="uint8")
    img[:, :] = (128, 128, 128)
    rr.log_image("rects_test/img", img)

    # 20 random rectangles
    rr.set_time_seconds("sim_time", 2)
    rects_xy = np.random.rand(20, 2) * 1024
    rects_wh = np.random.rand(20, 2) * (1024 - rects_xy + 1)
    rects = np.hstack((rects_xy, rects_wh))
    colors = np.array([[random.randrange(255) for _ in range(3)] for _ in range(20)])
    rr.log_rects("rects_test/rects", rects, colors=colors, rect_format=rr.RectFormat.XYWH)

    # Clear the rectangles by logging an empty set
    rr.set_time_seconds("sim_time", 3)
    rr.log_rects("rects_test/rects", [])


def run_text_logs(experimental_api: bool) -> None:
    rr.log_text_entry("logs", "Text with explicitly set color", color=[255, 215, 0], timeless=True)
    rr.log_text_entry("logs", "this entry has loglevel TRACE", level="TRACE")

    logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
    logging.getLogger().setLevel(-1)
    logging.info("This log got added through a `LoggingHandler`")


def run_log_cleared(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 1)
    rr.log_rect("null_test/rect/0", [5, 5, 4, 4], label="Rect1", color=(255, 0, 0))
    rr.log_rect("null_test/rect/1", [10, 5, 4, 4], label="Rect2", color=(0, 255, 0))
    rr.set_time_seconds("sim_time", 2)
    rr.log_cleared("null_test/rect/0")
    rr.set_time_seconds("sim_time", 3)
    rr.log_cleared("null_test/rect", recursive=True)
    rr.set_time_seconds("sim_time", 4)
    rr.log_rect("null_test/rect/0", [5, 5, 4, 4])
    rr.set_time_seconds("sim_time", 5)
    rr.log_rect("null_test/rect/1", [10, 5, 4, 4])


def transforms_rigid_3d(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 0)

    sun_to_planet_distance = 6.0
    planet_to_moon_distance = 3.0
    rotation_speed_planet = 2.0
    rotation_speed_moon = 5.0

    # Planetary motion is typically in the XY plane.
    rr.log_view_coordinates("transforms3d", up="+Z", timeless=True)
    rr.log_view_coordinates("transforms3d/sun", up="+Z", timeless=True)
    rr.log_view_coordinates("transforms3d/sun/planet", up="+Z", timeless=True)
    rr.log_view_coordinates("transforms3d/sun/planet/moon", up="+Z", timeless=True)

    # All are in the center of their own space:
    rr.log_point("transforms3d/sun", [0.0, 0.0, 0.0], radius=1.0, color=[255, 200, 10])
    rr.log_point("transforms3d/sun/planet", [0.0, 0.0, 0.0], radius=0.4, color=[40, 80, 200])
    rr.log_point("transforms3d/sun/planet/moon", [0.0, 0.0, 0.0], radius=0.15, color=[180, 180, 180])

    # "dust" around the "planet" (and inside, don't care)
    # distribution is quadratically higher in the middle
    radii = np.random.rand(200) * planet_to_moon_distance * 0.5
    angles = np.random.rand(200) * math.tau
    height = np.power(np.random.rand(200), 0.2) * 0.5 - 0.5
    rr.log_points(
        "transforms3d/sun/planet/dust",
        np.array([np.sin(angles) * radii, np.cos(angles) * radii, height]).transpose(),
        colors=[80, 80, 80],
        radii=0.025,
    )

    # paths where the planet & moon move
    angles = np.arange(0.0, 1.01, 0.01) * math.tau
    circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
    rr.log_line_strip(
        "transforms3d/sun/planet_path",
        circle * sun_to_planet_distance,
    )
    rr.log_line_strip(
        "transforms3d/sun/planet/moon_path",
        circle * planet_to_moon_distance,
    )

    # movement via transforms
    for i in range(0, 6 * 120):
        time = i / 120.0
        rr.set_time_seconds("sim_time", time)

        if experimental_api:
            import rerun.experimental as rr2
            from rerun.experimental import dt as rrd

            rr2.log(
                "transforms3d/sun/planet",
                rrd.TranslationRotationScale3D(
                    translation=[
                        math.sin(time * rotation_speed_planet) * sun_to_planet_distance,
                        math.cos(time * rotation_speed_planet) * sun_to_planet_distance,
                        0.0,
                    ],
                    rotation=rrd.RotationAxisAngle(axis=(1, 0, 0), angle=rrd.Angle(deg=20)),
                ),
            )
            rr2.log(
                "transforms3d/sun/planet/moon",
                rrd.TranslationRotationScale3D(
                    translation=[
                        math.cos(time * rotation_speed_moon) * planet_to_moon_distance,
                        math.sin(time * rotation_speed_moon) * planet_to_moon_distance,
                        0.0,
                    ],
                    from_parent=True,
                ),
            )
        else:
            rr.log_transform3d(
                "transforms3d/sun/planet",
                rr.TranslationRotationScale3D(
                    [
                        math.sin(time * rotation_speed_planet) * sun_to_planet_distance,
                        math.cos(time * rotation_speed_planet) * sun_to_planet_distance,
                        0.0,
                    ],
                    rr.RotationAxisAngle((1, 0, 0), degrees=20),
                ),
            )
            rr.log_transform3d(
                "transforms3d/sun/planet/moon",
                rr.TranslationRotationScale3D(
                    [
                        math.cos(time * rotation_speed_moon) * planet_to_moon_distance,
                        math.sin(time * rotation_speed_moon) * planet_to_moon_distance,
                        0.0,
                    ]
                ),
                from_parent=True,
            )


def run_bounding_box(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 0)
    rr.log_obb(
        "bbox_test/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([0.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[0, 255, 0],
        stroke_width=0.01,
        label="box/t0",
    )

    rr.set_time_seconds("sim_time", 1)
    rr.log_obb(
        "bbox_test/bbox",
        half_size=[1.0, 0.5, 0.25],
        position=np.array([1.0, 0.0, 0.0]),
        rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
        color=[255, 255, 0],
        stroke_width=0.02,
        label="box/t1",
    )


def run_extension_component(experimental_api: bool) -> None:
    rr.set_time_seconds("sim_time", 0)
    # Hack to establish 2d view bounds
    rr.log_rect("extension_components", [0, 0, 128, 128])

    # Single point
    rr.log_point("extension_components/point", np.array([64, 64]), color=(255, 0, 0))
    # Separate extension component
    rr.log_extension_components("extension_components/point", {"confidence": 0.9})

    # Batch points with extension
    # Note: each extension component must either be length 1 (a splat) or the same length as the batch
    rr.set_time_seconds("sim_time", 1)
    rr.log_points(
        "extension_components/points",
        np.array([[32, 32], [32, 96], [96, 32], [96, 96]]),
        colors=(0, 255, 0),
        ext={"corner": ["upper left", "lower left", "upper right", "lower right"], "training": True},
    )


def run_image_tensors(experimental_api: bool) -> None:
    # Make sure you use a colorful image with alpha!
    dir_path = os.path.dirname(os.path.realpath(__file__))
    img_path = f"{dir_path}/../../../crates/re_ui/data/logo_dark_mode.png"
    img_bgra = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)

    img_rgba = cv2.cvtColor(img_bgra, cv2.COLOR_BGRA2RGBA)
    rr.log_image("img_rgba", img_rgba)
    img_rgb = cv2.cvtColor(img_rgba, cv2.COLOR_RGBA2RGB)
    rr.log_image("img_rgb", img_rgb)
    img_gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY)
    rr.log_image("img_gray", img_gray)

    dtypes = [
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "int8",  # produces wrap-around when casting, producing ugly images, but clipping which is not useful as a test
        "int16",
        "int32",
        "int64",
        # "float16", // TODO(#2792): support f16 again
        "float32",
        "float64",
    ]

    for dtype in dtypes:
        rr.log_image(f"img_rgba_{dtype}", img_rgba.astype(dtype))
        rr.log_image(f"img_rgb_{dtype}", img_rgb.astype(dtype))
        rr.log_image(f"img_gray_{dtype}", img_gray.astype(dtype))


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
    parser.add_argument(
        "--experimental-api",
        dest="experimental_api",
        action="store_true",
        help="If specified run tests using the new experimental APIs",
    )

    random.seed(0)  # For deterministic results
    np.random.seed(0)  # For deterministic results

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
                    args=(
                        test,
                        rec,
                    ),
                )
                t.start()
                threads.append(t)
            else:
                logging.info(f"Starting {name}")
                with rec:
                    test(args.experimental_api)

        for t in threads:
            t.join()
    else:
        if args.split_recordings:
            with rr.script_setup(args, f"rerun_example_test_api/{args.test}"):
                tests[args.test](args.experimental_api)
        else:
            tests[args.test](args.experimental_api)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
