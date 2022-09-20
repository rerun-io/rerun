#!/usr/bin/env python3
"""Example using an example depth dataset from NYU: https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html

Setup:
``` sh
wget -P examples/nyud http://horatio.cs.nyu.edu/mit/silberman/nyu_depth_v2/cafe.zip
```

Run:
``` sh
python examples/nyud/main.py --folder-idx=0 examples/nyud/cafe.zip
```

Within the dataset are 3 subsets, corresponding to `--folder-idx` argument values `0-2`.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Final, Tuple
import zipfile
from datetime import datetime
import cv2
import numpy as np

import rerun_sdk as rerun

# Logging depth images is slow, so we don't log every frame
DEPTH_IMAGE_INTERVAL: Final = 8
DEPTH_IMAGE_SCALING: Final = 1e4


def parse_timestamp(filename: str) -> datetime:
    """Parse the timestamp portion of the filename."""
    file_name_parts = filename.split("-")
    time = file_name_parts[len(file_name_parts) - 2]
    return datetime.fromtimestamp(float(time))


def camera_for_image(h: float, w: float) -> Tuple[float, float, float]:
    """Returns a tuple of (u_center, v_center, focal_length) for a camera image."""
    return (w / 2, h / 2, 0.7 * w)


def camera_intrinsics(image: np.ndarray) -> np.ndarray:
    """Create reasonable camera intrinsics given the resolution."""
    (h, w) = image.shape
    (u_center, v_center, focal_length) = camera_for_image(h, w)
    return np.array(
        ((focal_length, 0, u_center), (0, focal_length, v_center), (0, 0, 1)))


def back_project(depth_image: np.ndarray) -> np.ndarray:
    """Given a depth image, generate a matching point cloud."""
    (h, w) = depth_image.shape
    (u_center, v_center, focal_length) = camera_for_image(h, w)

    # Pre-generate image containing the x and y coordinates per pixel
    u_coords, v_coords = np.meshgrid(np.arange(0, w), np.arange(0, h))

    # Apply inverse of the intrinsics matrix:
    z = depth_image.reshape(-1)
    x = (u_coords.reshape(-1).astype(float) - u_center) * z / focal_length
    y = (v_coords.reshape(-1).astype(float) - v_center) * z / focal_length

    back_projected = np.vstack((x, y, z)).T
    return back_projected


def read_image_rgb(buf: bytes) -> np.ndarray:
    """Decode an image provided in `buf`, and interpret it as RGB data."""
    np_buf: np.ndarray = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    # OpenCV reads images in BGR rather than RGB format
    img_bgr = cv2.imdecode(np_buf, cv2.IMREAD_COLOR)
    img_rgb: np.ndarray = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2RGB)
    return img_rgb


def read_image(buf: bytes) -> np.ndarray:
    """Decode an image provided in `buf`."""
    np_buf: np.ndarray = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    img: np.ndarray = cv2.imdecode(np_buf, cv2.IMREAD_UNCHANGED)
    return img


def log_nyud_data(dataset: Path, dir_idx: int = 0) -> None:
    depth_images_counter = 0

    rerun.set_space_up("world", [0, -1, 0])

    with zipfile.ZipFile(dataset, "r") as archive:
        archive_dirs = [f.filename for f in archive.filelist if f.is_dir()]
        dir_to_log = archive_dirs[dir_idx]
        files_to_process = [
            f for f in archive.filelist
            if f.filename.startswith(dir_to_log) and (
                f.filename.endswith(".ppm") or f.filename.endswith(".pgm"))
        ]

        for f in files_to_process:
            time = parse_timestamp(f.filename)
            rerun.set_time_seconds("time", time.timestamp())

            if f.filename.endswith(".ppm"):
                buf = archive.read(f)
                img_rgb = read_image_rgb(buf)
                rerun.log_image("rgb", img_rgb, space="image")

            elif f.filename.endswith(".pgm"):
                if depth_images_counter % DEPTH_IMAGE_INTERVAL == 0:
                    buf = archive.read(f)
                    img_depth = read_image(buf)
                    rerun.log_depth_image("depth",
                                          img_depth,
                                          meter=DEPTH_IMAGE_SCALING,
                                          space="image")

                    point_cloud = back_project(depth_image=img_depth /
                                               DEPTH_IMAGE_SCALING)
                    rerun.log_points("points",
                                     point_cloud,
                                     colors=np.array([255, 255, 255, 255]),
                                     space="world")

                    rerun.log_camera(
                        "camera",
                        resolution=img_depth.shape,
                        intrinsics=camera_intrinsics(img_depth),
                        rotation_q=np.array((0, 0, 0, 1)),
                        position=np.array((0, 0, 0)),
                        camera_space_convention=rerun.CameraSpaceConvention.
                        X_RIGHT_Y_DOWN_Z_FWD,
                        target_space="image",
                        space="world")

                depth_images_counter += 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description="Logs rich data using the Rerun SDK.")
    parser.add_argument("dataset",
                        type=Path,
                        help="Path to the cafe.zip archive.")
    parser.add_argument("--connect",
                        dest='connect',
                        action='store_true',
                        help="Connect to an external viewer")
    parser.add_argument("--addr",
                        type=str,
                        default=None,
                        help="Connect to this ip:port")
    parser.add_argument(
        "--folder-idx",
        type=int,
        default=0,
        help="The index of the folders within the dataset archive to log.")
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    log_nyud_data(
        dataset=args.dataset,
        dir_idx=args.folder_idx,
    )

    if not args.connect:
        # Show the logged data inside the Python process:
        rerun.show()
