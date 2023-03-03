#!/usr/bin/env python3
"""
Very simple example of capturing from a live webcam.

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

import cv2
import rerun as rr


def run_canny() -> None:
    # Create a new video capture
    cap = cv2.VideoCapture(0)

    while cap.isOpened():
        # Read the frame
        ret, img = cap.read()
        if not ret:
            print("Can't receive frame (stream end?). Exiting ...")
            break

        # Get the current frame time
        frame_time = cap.get(cv2.CAP_PROP_POS_MSEC)
        rr.set_time_nanos("frame_time", int(frame_time * 1000000))

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
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--device", type=int, default=0, help="Which camera device to use. (Passed to `cv2.VideoCapture()`)"
    )

    rr.script_add_args(parser)
    args = parser.parse_args()

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

    run_canny()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
