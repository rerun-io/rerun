#!/usr/bin/env python3
"""
A minimal example of streaming frames live from an intel realsense depth sensor.

NOTE: this example currently runs forever and will eventually exhaust your
system memory. It is advised you run an independent rerun viewer with a memory
limit:
```
rerun --memory-limit 4GB
```

And then connect using:
```
python examples/python/live_depth_sensor/main.py --connect
```

"""
from __future__ import annotations

import argparse

import numpy as np
import pyrealsense2 as rs
import rerun as rr  # pip install rerun-sdk


def run_realsense(num_frames: int | None) -> None:
    # Visualize the data as RDF
    rr.log_view_coordinates("realsense", xyz="RDF", timeless=True)

    # Open the pipe
    pipe = rs.pipeline()
    profile = pipe.start()

    # Get and log depth exstrinsics
    depth_profile = profile.get_stream(rs.stream.depth)
    depth_intr = depth_profile.as_video_stream_profile().get_intrinsics()

    rr.log_pinhole(
        "realsense/camera/depth",
        child_from_parent=np.array(
            (
                (depth_intr.fx, 0, depth_intr.ppx),
                (0, depth_intr.fy, depth_intr.ppy),
                (0, 0, 1),
            ),
        ),
        width=depth_intr.width,
        height=depth_intr.height,
        timeless=True,
    )

    # Get and log color extrinsics
    rgb_profile = profile.get_stream(rs.stream.color)

    depth_extr = depth_profile.get_extrinsics_to(rgb_profile)
    rr.log_transform3d(
        "realsense/camera/ext",
        transform=rr.TranslationAndMat3(
            translation=depth_extr.translation, matrix=np.reshape(depth_extr.rotation, (3, 3))
        ),
        from_parent=True,
        timeless=True,
    )

    # Get and log color intrinsics
    rgb_intr = rgb_profile.as_video_stream_profile().get_intrinsics()

    rr.log_pinhole(
        "realsense/camera/ext/rgb",
        child_from_parent=np.array(
            (
                (rgb_intr.fx, 0, rgb_intr.ppx),
                (0, rgb_intr.fy, rgb_intr.ppy),
                (0, 0, 1),
            ),
        ),
        width=rgb_intr.width,
        height=rgb_intr.height,
        timeless=True,
    )

    # Read frames in a loop
    frame_nr = 0
    try:
        while True:
            if num_frames and frame_nr >= num_frames:
                break

            rr.set_time_sequence("frame_nr", frame_nr)
            frame_nr += 1

            frames = pipe.wait_for_frames()
            for f in frames:
                # Log the depth frame
                depth_frame = frames.get_depth_frame()
                depth_units = depth_frame.get_units()
                depth_image = np.asanyarray(depth_frame.get_data())
                rr.log_depth_image("realsense/camera/depth", depth_image, meter=1.0 / depth_units)

                # Log the color frame
                color_frame = frames.get_color_frame()
                color_image = np.asanyarray(color_frame.get_data())
                rr.log_image("realsense/camera/ext/rgb", color_image)
    finally:
        pipe.stop()


def main() -> None:
    parser = argparse.ArgumentParser(description="Streams frames from a connected realsense depth sensor.")
    parser.add_argument("--num-frames", type=int, default=None, help="The number of frames to log")

    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "live_depth_sensor")

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
python examples/python/live_depth_sensor/main.py --connect
```
################################################################################
        """
        )

    run_realsense(args.num_frames)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
