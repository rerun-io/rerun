from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Final

import cv2
import requests

DATASET_BASE_URL = "https://storage.googleapis.com/objectron"
LOCAL_DATASET_DIR: Final = Path(__file__).parent.parent / "dataset"
IMAGE_RESOLUTION: Final = (1440, 1920)
GEOMETRY_FILENAME: Final = "geometry.pbdata"
ANNOTATIONS_FILENAME: Final = "annotation.pbdata"
VIDEO_FILENAME: Final = "video.MOV"

AVAILABLE_RECORDINGS = [
    "bike",
    "book",
    "bottle",
    "camera",
    "cereal_box",
    "chair",
    "cup",
    "laptop",
    "shoe",
]


def ensure_downloaded(src_url: str, dst_path: Path) -> None:
    os.makedirs(dst_path.parent, exist_ok=True)
    if not dst_path.exists():
        logging.info("Downloading %s to %s", src_url, dst_path)
        with requests.get(src_url, stream=True) as req:
            req.raise_for_status()
            with open(dst_path, "wb") as f:
                f.writelines(req.iter_content(chunk_size=8192))


def find_path_if_downloaded(recording_name: str, local_dataset_dir: Path) -> Path | None:
    local_recording_dir = local_dataset_dir / recording_name
    paths = list(local_recording_dir.glob(f"**/{ANNOTATIONS_FILENAME}"))
    if paths:
        return paths[0].parent
    return None


def get_recording_id_from_name(recording_name: str) -> str:
    recording_ids_raw = requests.get(f"{DATASET_BASE_URL}/v1/index/{recording_name}_annotations_test").text
    recording_id = recording_ids_raw.split("\n")[0]
    return recording_id


def ensure_opencv_version_ok() -> None:
    if cv2.getVersionMajor() == 4 and cv2.getVersionMinor() == 6:
        raise RuntimeError(
            """Opencv 4.6 contains a bug which will unpack some videos with the incorrect orientation.
                See: https://github.com/opencv/opencv/issues/22088
                Please upgrade or downgrade as appropriate.""",
        )


def ensure_recording_downloaded(recording_name: str, dataset_dir: Path) -> Path:
    """
    Makes sure the recording is downloaded.

    Returns the path to where the dataset is downloaded locally.
    """
    ensure_opencv_version_ok()

    local_recording_dir = find_path_if_downloaded(recording_name, dataset_dir)
    if local_recording_dir is not None:
        return local_recording_dir

    recording_id = get_recording_id_from_name(recording_name)
    local_recording_dir = dataset_dir / recording_id
    recording_url = f"{DATASET_BASE_URL}/videos/{recording_id}"

    ensure_downloaded(f"{recording_url}/{VIDEO_FILENAME}", local_recording_dir / VIDEO_FILENAME)
    ensure_downloaded(f"{recording_url}/{GEOMETRY_FILENAME}", local_recording_dir / GEOMETRY_FILENAME)
    ensure_downloaded(
        f"{DATASET_BASE_URL}/annotations/{recording_id}.pbdata",
        local_recording_dir / ANNOTATIONS_FILENAME,
    )

    return local_recording_dir


def ensure_video_is_split_into_frames(recording_dir: Path, force_reprocess: bool = False) -> None:
    video_path = recording_dir / VIDEO_FILENAME
    frames_dir = recording_dir / "video"
    if force_reprocess or not frames_dir.exists():
        logging.info("Splitting video at %s into frames in %s", video_path, frames_dir)
        os.makedirs(frames_dir, exist_ok=True)

        vidcap = cv2.VideoCapture(str(video_path))
        success, image = vidcap.read()
        count = 0
        while success:
            cv2.imwrite(f"{frames_dir}/{count}.jpg", image)
            success, image = vidcap.read()
            count += 1


def ensure_recording_available(name: str, local_dataset_dir: Path, force_reprocess_video: bool = False) -> Path:
    recording_path = ensure_recording_downloaded(name, local_dataset_dir)
    ensure_video_is_split_into_frames(recording_path, force_reprocess_video)
    return recording_path
