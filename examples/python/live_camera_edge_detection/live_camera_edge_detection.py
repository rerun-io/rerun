#!/usr/bin/env python3
"""
Very simple example of capturing from a live camera.

Runs the opencv canny edge detector on the image stream.
"""

from __future__ import annotations

import argparse

import cv2
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb


def run_canny(num_frames: int | None) -> None:
    # Create a new video capture
    cap = cv2.VideoCapture(0)

    frame_nr = 0

    while cap.isOpened():
        if num_frames and frame_nr >= num_frames:
            break

        # Read the frame
        ret, img = cap.read()
        if not ret:
            if frame_nr == 0:
                print("Failed to capture any frame. No camera connected?")
            else:
                print("Can't receive frame (stream end?). Exiting…")
            break

        # Get the current frame time. On some platforms it always returns zero.
        frame_time_ms = cap.get(cv2.CAP_PROP_POS_MSEC)
        if frame_time_ms != 0:
            rr.set_time("frame_time", duration=1e-3 * frame_time_ms)

        rr.set_time("frame_nr", sequence=frame_nr)
        frame_nr += 1

        # Log the original image
        for i in range(16):
            rr.log("image/rgb", rr.Image(img, color_model="BGR"))

        # Convert to grayscale
        gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
        rr.log("image/gray", rr.Image(gray))

        # Run the canny edge detector
        canny = cv2.Canny(gray, 50, 200)
        rr.log("image/canny", rr.Image(canny))


def main() -> None:
    parser = argparse.ArgumentParser(description="Streams a local system camera and runs the canny edge detector.")
    parser.add_argument(
        "--device",
        type=int,
        default=0,
        help="Which camera device to use. (Passed to `cv2.VideoCapture()`)",
    )
    parser.add_argument("--num-frames", type=int, default=None, help="The number of frames to log")

    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(
        args,
        "rerun_example_live_camera_edge_detection",
        default_blueprint=rrb.Vertical(
            rrb.Horizontal(
                rrb.Spatial2DView(origin="/image/rgb", name="Video"),
                rrb.Spatial2DView(origin="/image/gray", name="Video (Grayscale)"),
            ),
            rrb.Spatial2DView(origin="/image/canny", name="Canny Edge Detector"),
            row_shares=[1, 2],
        ),
    )

    run_canny(args.num_frames)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
