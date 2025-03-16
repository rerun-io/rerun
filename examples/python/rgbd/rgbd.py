#!/usr/bin/env python3
"""
Example using an example depth dataset from NYU.

https://cs.nyu.edu/~fergus/datasets/nyu_depth_v2.html
"""

from __future__ import annotations

import argparse
import os
import sys
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Final

import cv2
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
from tqdm import tqdm

DESCRIPTION = """
# RGBD
Visualizes an example recording from [the NYUD dataset](https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html) with RGB and Depth channels.

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/rgbd).
"""

DEPTH_IMAGE_SCALING: Final = 1e4
DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL_BASE: Final = "https://static.rerun.io/rgbd_dataset"
DATASET_URL_BASE_ALTERNATE: Final = "https://cs.nyu.edu/~fergus/datasets/nyu_depth_v2.html"
AVAILABLE_RECORDINGS: Final = ["cafe", "basements", "studies", "office_kitchens", "playroooms"]


def parse_timestamp(filename: str) -> datetime:
    """Parse the timestamp portion of the filename."""
    file_name_parts = filename.split("-")
    time = file_name_parts[len(file_name_parts) - 2]
    return datetime.fromtimestamp(float(time))


def read_image_bgr(buf: bytes) -> npt.NDArray[np.uint8]:
    """Decode an image provided in `buf`, and interpret it as RGB data."""
    np_buf: npt.NDArray[np.uint8] = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    img_bgr: npt.NDArray[Any] = cv2.imdecode(np_buf, cv2.IMREAD_COLOR)
    return img_bgr


def read_depth_image(buf: bytes) -> npt.NDArray[Any]:
    """Decode an image provided in `buf`."""
    np_buf: npt.NDArray[np.uint8] = np.ndarray(shape=(1, len(buf)), dtype=np.uint8, buffer=buf)
    img: npt.NDArray[Any] = cv2.imdecode(np_buf, cv2.IMREAD_UNCHANGED)
    return img


def log_nyud_data(recording_path: Path, subset_idx: int, frames: int) -> None:
    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, static=True)

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

        if len(files_with_timestamps) > frames:
            files_with_timestamps = files_with_timestamps[:frames]

        for time, f in files_with_timestamps:
            rr.set_time("time", timestamp=time.timestamp())

            if f.filename.endswith(".ppm"):
                buf = archive.read(f)
                img_bgr = read_image_bgr(buf)
                rr.log("world/camera/image/rgb", rr.Image(img_bgr, color_model="BGR").compress(jpeg_quality=95))

            elif f.filename.endswith(".pgm"):
                buf = archive.read(f)
                img_depth = read_depth_image(buf)

                # Log the camera transforms:
                rr.log(
                    "world/camera/image",
                    rr.Pinhole(
                        resolution=[img_depth.shape[1], img_depth.shape[0]],
                        focal_length=0.7 * img_depth.shape[1],
                        # Intentionally off-center to demonstrate that we support it
                        principal_point=[0.45 * img_depth.shape[1], 0.55 * img_depth.shape[0]],
                    ),
                )

                # Log the depth image to the cameras image-space:
                rr.log("world/camera/image/depth", rr.DepthImage(img_depth, meter=DEPTH_IMAGE_SCALING))


def ensure_recording_downloaded(name: str) -> Path:
    recording_filename = f"{name}.zip"
    recording_path = DATASET_DIR / recording_filename
    if recording_path.exists():
        return recording_path

    url = f"{DATASET_URL_BASE}/{recording_filename}"
    alternate_url = f"{DATASET_URL_BASE_ALTERNATE}/{recording_filename}"

    os.makedirs(DATASET_DIR, exist_ok=True)
    try:
        try:
            print(f"downloading {url} to {recording_path}")
            download_progress(url, recording_path)
        except ValueError:
            print(f"Failed to download from {url}, trying backup URL {alternate_url} instead")
            download_progress(alternate_url, recording_path)
    except BaseException as e:
        recording_path.unlink(missing_ok=True)
        raise e

    return recording_path


def download_progress(url: str, dst: Path) -> None:
    """
    Download file with tqdm progress bar.

    From: <https://gist.github.com/yanqd0/c13ed29e29432e3cf3e7c38467f42f51>
    """
    resp = requests.get(url, stream=True)
    if resp.status_code != 200:
        raise ValueError(f"Failed to download file (status code: {resp.status_code})")
    total = int(resp.headers.get("content-length", 0))
    chunk_size = 1024 * 1024
    # Can also replace 'file' with a io.BytesIO object
    with (
        open(dst, "wb") as file,
        tqdm(
            desc=dst.name,
            total=total,
            unit="iB",
            unit_scale=True,
            unit_divisor=1024,
        ) as bar,
    ):
        for data in resp.iter_content(chunk_size=chunk_size):
            size = file.write(data)
            bar.update(size)


def main() -> None:
    parser = argparse.ArgumentParser(description="Example using an example depth dataset from NYU.")
    parser.add_argument(
        "--recording",
        type=str,
        choices=AVAILABLE_RECORDINGS,
        default=AVAILABLE_RECORDINGS[0],
        help="Name of the NYU Depth Dataset V2 recording",
    )
    parser.add_argument("--subset-idx", type=int, default=0, help="The index of the subset of the recording to use.")
    parser.add_argument(
        "--frames",
        type=int,
        default=sys.maxsize,
        help="If specified, limits the number of frames logged",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(
        args,
        "rerun_example_rgbd",
        default_blueprint=rrb.Horizontal(
            rrb.Vertical(
                rrb.Spatial3DView(name="3D", origin="world"),
                rrb.TextDocumentView(name="Description", origin="/description"),
                row_shares=[7, 3],
            ),
            rrb.Vertical(
                # Put the origin for both 2D spaces where the pinhole is logged. Doing so allows them to understand how they're connected to the 3D space.
                # This enables interactions like clicking on a point in the 3D space to show the corresponding point in the 2D spaces and vice versa.
                rrb.Spatial2DView(
                    name="RGB & Depth",
                    origin="world/camera/image",
                    overrides={"world/camera/image/rgb": rr.Image.from_fields(opacity=0.5)},
                ),
                rrb.Tabs(
                    rrb.Spatial2DView(name="RGB", origin="world/camera/image", contents="world/camera/image/rgb"),
                    rrb.Spatial2DView(name="Depth", origin="world/camera/image", contents="world/camera/image/depth"),
                ),
                name="2D",
                row_shares=[3, 3, 2],
            ),
            column_shares=[2, 1],
        ),
    )

    recording_path = ensure_recording_downloaded(args.recording)

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    log_nyud_data(
        recording_path=recording_path,
        subset_idx=args.subset_idx,
        frames=args.frames,
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
