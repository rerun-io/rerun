#!/usr/bin/env python3
"""Example of using Rerun to log and visualize the output of COLMAP's sparse reconstruction."""
import io
import os
import zipfile
from argparse import ArgumentParser
from pathlib import Path
from typing import Any, Final

from PIL import Image
import numpy as np
import numpy.typing as npt
import requests
from read_write_model import Camera, read_model
from tqdm import tqdm
from transformers import pipeline


import rerun as rr

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/colmap"
DATASET_NAME: Final = "colmap_rusty_car"
DATASET_URL: Final = f"{DATASET_URL_BASE}/{DATASET_NAME}.zip"
# When dataset filtering is turned on, drop views with less than this many valid points.
FILTER_MIN_VISIBLE: Final = 500


RED: Final = (255, 105, 129)
GREEN: Final = (0, 178, 102)
BLUE: Final = (122, 149, 255)


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

    rr.log_view_coordinates("world", up="-Y", timeless=True)

    obj_detector = pipeline("object-detection")

    # Iterate through images (video frames) logging data related to each frame.
    seen_ids = np.ndarray((0,), dtype="int64")
    for image in sorted(images.values(), key=lambda im: im.name):  # type: ignore[no-any-return]
        frame_idx = int(image.name[0:4])  # COLMAP sets image ids that don't match the original video frame
        quat_xyzw = image.qvec[[1, 2, 3, 0]]  # COLMAP uses wxyz quaternions
        camera_from_world = (image.tvec, quat_xyzw)  # COLMAP's camera transform is "camera from world"
        camera = cameras[image.camera_id]
        intrinsics = intrinsics_for_camera(camera)

        visible = [id != -1 and points3D.get(id) is not None for id in image.point3D_ids]
        visible_ids = image.point3D_ids[visible]
        seen_ids = np.unique(np.hstack([visible_ids, seen_ids]))

        if filter_output and len(visible_ids) < FILTER_MIN_VISIBLE:
            continue

        seen_xyzs = [points3D[id] for id in seen_ids]
        visible_xyzs = [points3D[id] for id in visible_ids]
        visible_xys = image.xys[visible]

        rr.set_time_sequence("frame", frame_idx)

        seen_points = [point.xyz for point in seen_xyzs]
        seen_colors = [point.rgb for point in seen_xyzs]

        point_colors = [point.rgb for point in visible_xyzs]

        rr.log_points("points", seen_points, colors=seen_colors)

        rr.log_rigid3(
            "world/cam",
            child_from_parent=camera_from_world,
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        rr.log_scalar("cam/x", image.tvec[0], label="x", color=RED)
        rr.log_scalar("cam/y", image.tvec[1], label="y", color=GREEN)
        rr.log_scalar("cam/z", image.tvec[2], label="z", color=BLUE)

        # Log camera intrinsics
        rr.log_pinhole(
            "world/cam/img",
            child_from_parent=intrinsics,
            width=camera.width,
            height=camera.height,
        )

        image_path = dataset_path / "images" / image.name
        image = Image.open(image_path)

        rr.log_image("world/cam/img/rgb", image)

        detections = obj_detector(image)

        if len(detections) > 0:
            box = detections[0]["box"]
            bbox = np.array([box["xmin"], box["ymin"], box["xmax"], box["ymax"]])

            rr.log_rects(
                "world/cam/img/detection",
                bbox,
                labels=["car"],
                rect_format=rr.log.rects.RectFormat.XYXY,
                colors=(255, 255, 255),
            )

        rr.log_points("world/cam/img/keypoints", visible_xys, colors=point_colors)


def main() -> None:
    parser = ArgumentParser(description="Visualize the output of COLMAP's sparse reconstruction on a video.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    parser.add_argument("--unfiltered", action="store_true", help="If set, we don't filter away any noisy data.")
    args = parser.parse_args()

    rr.init("colmap")

    if args.serve:
        rr.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    dataset_path = get_downloaded_dataset_path()

    read_and_log_sparse_reconstruction(dataset_path, filter_output=not args.unfiltered)

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep

            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rr.save(args.save)


if __name__ == "__main__":
    main()
