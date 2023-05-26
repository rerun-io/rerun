#!/usr/bin/env python3
"""
Very simple example of capturing from a live camera.

Runs the opencv canny edge detector on the image stream.

Known issues:
 - The example has 2+ seconds latency on macOS. [#2216](https://github.com/rerun-io/rerun/issues/2216)

NOTE: this example currently runs forever and will eventually exhaust your
system memory. It is advised you run an independent rerun viewer with a memory
limit:
```
rerun --memory-limit 4GB
```

And then connect using:
```
python examples/python/opencv_canny/main.py --connect
```

"""

import argparse
from typing import Optional

import cv2
import rerun as rr  # pip install rerun-sdk


def run_canny(num_frames: Optional[int]) -> None:
    # Create a new video capture
    cap = cv2.VideoCapture(0)

    frame_nr = 0

    while cap.isOpened():
        if num_frames and frame_nr >= num_frames:
            break

        # Read the frame
        ret, img = cap.read()
        if not ret:
            print("Can't receive frame (stream end?). Exiting ...")
            break

        # Get the current frame time. On some platforms it always returns zero.
        frame_time_ms = cap.get(cv2.CAP_PROP_POS_MSEC)
        if frame_time_ms != 0:
            rr.set_time_nanos("frame_time", int(frame_time_ms * 1_000_000))

        rr.set_time_sequence("frame_nr", frame_nr)
        frame_nr += 1

        # Log the original image
        rgb = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
        rr.log_image("image/rgb", rgb)

        # Convert to grayscale
        gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
        rr.log_image("image/gray", gray)

        # Run the canny edge detector
        canny = cv2.Canny(gray, 50, 200)
        rr.log_image("image/canny", canny)


def main() -> None:
    parser = argparse.ArgumentParser(description="Streams a local system camera and runs the canny edge detector.")
    parser.add_argument(
        "--device", type=int, default=0, help="Which camera device to use. (Passed to `cv2.VideoCapture()`)"
    )
    parser.add_argument("--num-frames", type=int, default=None, help="The number of frames to log")

    rr.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.script_setup(args, "opencv_canny")

    print(args.connect)

    if not args.connect:
        print(
            """
################################################################################
NOTE: this example currently runs forever and will eventually exhaust your
system memory. It is advised you run an independent rerun viewer with a memory
limit:
```
rerun --memory-limit 4GB
```

And then connect using:
```
python examples/python/opencv_canny/main.py --connect
```
################################################################################
        """
        )

    run_canny(args.num_frames)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
