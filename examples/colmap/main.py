#!/usr/bin/env python3
"""Example of using Rerun to log and visualize the output of COLMAP's sparse reconstruction."""
import io
import os
import zipfile
from argparse import ArgumentParser
from pathlib import Path
from typing import Any, Final

import numpy as np
import numpy.typing as npt
import requests
from read_write_model import Camera, read_model
from tqdm import tqdm

import rerun as rr

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/colmap"
DATASET_NAME: Final = "colmap_rusty_car"
DATASET_URL: Final = f"{DATASET_URL_BASE}/{DATASET_NAME}.zip"
# When dataset filtering is turned on, drop views with less than this many valid points.
FILTER_MIN_VISIBLE: Final = 500


def intrinsics_for_camera(camera: Camera) -> npt.NDArray[Any]:
    """Convert a colmap camera to a pinhole camera intrinsics matrix."""
    return np.vstack(
        [
            np.hstack(
                [
                    # Focal length is in [:2]
                    np.diag(camera.params[:2]),
                    # Principle point is in [2:]
                    np.vstack(camera.params[2:]),
                ]
            ),
            [0, 0, 1],
        ]
    )


def get_downloaded_dataset_path() -> Path:
    recording_dir = DATASET_DIR / DATASET_NAME
    if recording_dir.exists():
        return recording_dir

    os.makedirs(DATASET_DIR, exist_ok=True)

    zip_file = download_with_progress(DATASET_URL)

    with zipfile.ZipFile(zip_file) as zip_ref:
        progress = tqdm(zip_ref.infolist(), "Extracting dataset", total=len(zip_ref.infolist()), unit="files")
        for file in progress:
            zip_ref.extract(file, DATASET_DIR)
            progress.update()

    return recording_dir


def download_with_progress(url: str) -> io.BytesIO:
    """Download file with tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with tqdm(
        desc=f"Downloading dataset", total=total_size, unit="B", unit_scale=True, unit_divisor=chunk_size
    ) as progress:
        zip_file = io.BytesIO()
        for data in resp.iter_content(chunk_size):
            zip_file.write(data)
            progress.update(len(data))

    zip_file.seek(0)
    return zip_file


def read_and_log_sparse_reconstruction(dataset_path: Path, filter_output: bool) -> None:
    print("Reading sparse COLMAP reconstruction")
    cameras, images, points3D = read_model(dataset_path / "sparse", ext=".bin")
    print("Building visualization by logging to Rerun")

    if filter_output:
        # Filter out noisy points
        points3D = {id: point for id, point in points3D.items() if point.rgb.any() and len(point.image_ids) > 4}

    rr.log_view_coordinates("/", up="-Y", timeless=True)

    # Iterate through images (video frames) logging data related to each frame.
    for image in sorted(images.values(), key=lambda im: im.name):  # type: ignore[no-any-return]
        frame_idx = int(image.name[0:4])  # COLMAP sets image ids that don't match the original video frame
        quat_xyzw = image.qvec[[1, 2, 3, 0]]  # COLMAP uses wxyz quaternions
        camera_from_world = (image.tvec, quat_xyzw)  # COLMAP's camera transform is "camera from world"
        camera = cameras[image.camera_id]
        intrinsics = intrinsics_for_camera(camera)

        visible = [id != -1 and points3D.get(id) is not None for id in image.point3D_ids]
        visible_ids = image.point3D_ids[visible]

        if filter_output and len(visible_ids) < FILTER_MIN_VISIBLE:
            continue

        visible_xyzs = [points3D[id] for id in visible_ids]
        visible_xys = image.xys[visible]

        rr.set_time_sequence("frame", frame_idx)

        points = [point.xyz for point in visible_xyzs]
        point_colors = [point.rgb for point in visible_xyzs]

        rr.log_points("points", points, colors=point_colors)

        rr.log_rigid3(
            "camera",
            child_from_parent=camera_from_world,
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        # Log camera intrinsics
        rr.log_pinhole(
            "camera/image",
            child_from_parent=intrinsics,
            width=camera.width,
            height=camera.height,
        )

        rr.log_image_file("camera/image/rgb", img_path=dataset_path / "images" / image.name)

        rr.log_points("camera/image/keypoints", visible_xys, colors=point_colors)


def main() -> None:
    parser = ArgumentParser(description="Visualize the output of COLMAP's sparse reconstruction on a video.")
    parser.add_argument("--unfiltered", action="store_true", help="If set, we don't filter away any noisy data.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "colmap")
    dataset_path = get_downloaded_dataset_path()
    read_and_log_sparse_reconstruction(dataset_path, filter_output=not args.unfiltered)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
