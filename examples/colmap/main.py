#!/usr/bin/env python3
"""

"""
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
    chunk_size = 8192
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


@rr.script("Visualize Colmap Data")
def main(parser: ArgumentParser) -> None:
    args = parser.parse_args()

    dataset_path = get_downloaded_dataset_path()
    print("Loading sparse colmap reconstruction")
    cameras, images, points3D = read_model(dataset_path / "sparse")

    rr.init("colmap", spawn_and_connect=True)
    rr.log_view_coordinates("world", up="-Y", timeless=True)

    # Filter out noisy points
    filtered = {id: point for id, point in points3D.items() if point.rgb.any() and len(point.image_ids) > 4}

    for image in sorted(images.values(), key=lambda im: im.name):  # type: ignore[no-any-return]
        frame_idx = int(image.name[0:4])  # COLMAP sets image ids that don't match the original video frame
        quat_xyzw = image.qvec[[1, 2, 3, 0]]  # COLMAP uses wxyz quaternions
        camera_from_world = (image.tvec, quat_xyzw)  # COLMAP's camera transform is "camera from world"
        camera = cameras[image.camera_id]
        intrinsics = intrinsics_for_camera(camera)

        visible_points = [filtered.get(id) for id in image.point3D_ids if id != -1]
        visible_points = [point for point in visible_points if point is not None]

        rr.set_time_sequence("frame", frame_idx)

        points = [point.xyz for point in visible_points]  # type: ignore[union-attr]
        point_colors = [point.rgb for point in visible_points]  # type: ignore[union-attr]
        rr.log_points(f"world/points", points, colors=point_colors)

        rr.log_rigid3(
            f"world/camera",
            child_from_parent=camera_from_world,
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        # Log camera intrinsics
        rr.log_pinhole(
            f"world/camera/image",
            child_from_parent=intrinsics,
            width=camera.width,
            height=camera.height,
        )

        rr.log_image_file(f"world/camera/image/rgb", dataset_path / "images" / image.name)

        rr.log_points(f"world/camera/image/keypoints", image.xys)


if __name__ == "__main__":
    main()
