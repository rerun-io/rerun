#!/usr/bin/env python3
"""
Example using an example depth dataset from NYU.

https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html
"""

import argparse
import os
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Final, Tuple

import cv2
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk
from tqdm import tqdm

DEPTH_IMAGE_SCALING: Final = 1e4
DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL_BASE: Final = "http://horatio.cs.nyu.edu/mit/silberman/nyu_depth_v2"
AVAILABLE_RECORDINGS: Final = ["cafe", "basements", "studies", "office_kitchens", "playroooms"]


def parse_timestamp(filename: str) -> datetime:
    """Parse the timestamp portion of the filename."""
    file_name_parts = filename.split("-")
    time = file_name_parts[len(file_name_parts) - 2]
    return datetime.fromtimestamp(float(time))


def camera_for_image(h: float, w: float) -> Tuple[float, float, float]:
    """Returns a tuple of (u_center, v_center, focal_length) for a camera image."""
    return (w / 2, h / 2, 0.7 * w)


def camera_intrinsics(image: npt.NDArray[np.uint8]) -> npt.NDArray[np.uint8]:
    """Create reasonable camera intrinsics given the resolution."""
    (h, w) = image.shape
    (u_center, v_center, focal_length) = camera_for_image(h, w)
    return np.array(((focal_length, 0, u_center), (0, focal_length, v_center), (0, 0, 1)))


def read_image_rgb(buf: bytes) -> npt.NDArray[np.uint8]:
    """Decode an image provided in `buf`, and interpret it as RGB data."""
    np_buf: npt.NDArray[np.uint8] = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    # OpenCV reads images in BGR rather than RGB format
    img_bgr = cv2.imdecode(np_buf, cv2.IMREAD_COLOR)
    img_rgb: npt.NDArray[np.uint8] = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2RGB)
    return img_rgb


def read_image(buf: bytes) -> npt.NDArray[np.uint8]:
    """Decode an image provided in `buf`."""
    np_buf: npt.NDArray[np.uint8] = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    img: npt.NDArray[np.uint8] = cv2.imdecode(np_buf, cv2.IMREAD_UNCHANGED)
    return img


def log_nyud_data(recording_path: Path, subset_idx: int = 0) -> None:
    rr.log_view_coordinates("world", up="-Y", timeless=True)

    with zipfile.ZipFile(recording_path, "r") as archive:
        archive_dirs = [f.filename for f in archive.filelist if f.is_dir()]

        print(f"Using recording subset {subset_idx} ([0 - {len(archive_dirs) - 1}] available).")

        dir_to_log = archive_dirs[subset_idx]
        subset = [
            f
            for f in archive.filelist
            if f.filename.startswith(dir_to_log) and (f.filename.endswith(".ppm") or f.filename.endswith(".pgm"))
        ]
        files_with_timestamps = [(parse_timestamp(f.filename), f) for f in subset]
        files_with_timestamps.sort(key=lambda t: t[0])

        for time, f in files_with_timestamps:
            rr.set_time_seconds("time", time.timestamp())

            if f.filename.endswith(".ppm"):
                buf = archive.read(f)
                img_rgb = read_image_rgb(buf)
                rr.log_image("world/camera/image/rgb", img_rgb)

            elif f.filename.endswith(".pgm"):
                buf = archive.read(f)
                img_depth = read_image(buf)

                # Log the camera transforms:
                rr.log_view_coordinates("world/camera", xyz="RDF")  # X=Right, Y=Down, Z=Forward
                rr.log_pinhole(
                    "world/camera/image",
                    child_from_parent=camera_intrinsics(img_depth),
                    width=img_depth.shape[1],
                    height=img_depth.shape[0],
                )

                # Log the depth image to the cameras image-space:
                rr.log_depth_image("world/camera/image/depth", img_depth, meter=DEPTH_IMAGE_SCALING)


def ensure_recording_downloaded(name: str) -> Path:
    recording_filename = f"{name}.zip"
    recording_path = DATASET_DIR / recording_filename
    if recording_path.exists():
        return recording_path

    url = f"{DATASET_URL_BASE}/{recording_filename}"
    print(f"downloading {url} to {recording_path}")
    os.makedirs(DATASET_DIR, exist_ok=True)
    try:
        download_progress(url, recording_path)
    except BaseException as e:
        if recording_path.exists():
            os.remove(recording_path)
        raise e

    return recording_path


def download_progress(url: str, dst: Path) -> None:
    """
    Download file with tqdm progress bar.

    From: https://gist.github.com/yanqd0/c13ed29e29432e3cf3e7c38467f42f51
    """
    resp = requests.get(url, stream=True)
    total = int(resp.headers.get("content-length", 0))
    chunk_size = 1024 * 1024
    # Can also replace 'file' with a io.BytesIO object
    with open(dst, "wb") as file, tqdm(
        desc=dst.name,
        total=total,
        unit="iB",
        unit_scale=True,
        unit_divisor=1024,
    ) as bar:
        for data in resp.iter_content(chunk_size=chunk_size):
            size = file.write(data)
            bar.update(size)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--recording",
        type=str,
        choices=AVAILABLE_RECORDINGS,
        default=AVAILABLE_RECORDINGS[0],
        help="Name of the NYU Depth Dataset V2 recording",
    )
    parser.add_argument("--subset-idx", type=int, default=0, help="The index of the subset of the recording to use.")
    rr.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.script_setup(args, "nyud")
    recording_path = ensure_recording_downloaded(args.recording)

    log_nyud_data(
        recording_path=recording_path,
        subset_idx=args.subset_idx,
    )

    rr.script_teardown(args)
