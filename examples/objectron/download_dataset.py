#!/usr/bin/env python3

import argparse
import os
from pathlib import Path

import cv2
import requests

DATASET_BASE_URL = "https://storage.googleapis.com/objectron"


def download(url: str, path: str) -> None:
    if not os.path.exists(path):
        print(f"downloading {url}…")
        response = requests.get(url)
        with open(path, "wb") as file:
            file.write(response.content)


def split_video_into_frames(video_path: str, frames_path: str, reprocess_video: bool) -> None:
    if not os.path.exists(frames_path) or reprocess_video:
        print("Splitting video into frames…")
        os.makedirs(frames_path, exist_ok=True)

        vidcap = cv2.VideoCapture(video_path)
        success, image = vidcap.read()
        count = 0
        while success:
            cv2.imwrite(f"{frames_path}/{count}.jpg", image)
            success, image = vidcap.read()
            count += 1


def download_data(video_id: str, reprocess_video: bool) -> None:
    print(f"downloading {video_id}…")

    dir = f"dataset/{video_id}"
    os.makedirs(dir, exist_ok=True)

    download(f"{DATASET_BASE_URL}/videos/{video_id}/video.MOV", f"{dir}/video.MOV")

    # use object.proto
    download(f"{DATASET_BASE_URL}/videos/{video_id}/geometry.pbdata", f"{dir}/geometry.pbdata")

    # Please refer to Parse Annotation tutorial to see how to parse the annotation files.
    download(f"{DATASET_BASE_URL}/annotations/{video_id}.pbdata", f"{dir}/annotation.pbdata")

    split_video_into_frames(f"{dir}/video.MOV", f"{dir}/video", reprocess_video)


def download_dataset(name: str, reprocess_video: bool) -> None:
    video_ids_raw = requests.get(f"{DATASET_BASE_URL}/v1/index/{name}_annotations_test").text
    video_id = video_ids_raw.split("\n")[0]
    download_data(video_id, reprocess_video)


if __name__ == "__main__":
    available_datasets = [
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

    parser = argparse.ArgumentParser(description="Download the objectron datasets")
    parser.add_argument(
        "--force-reprocess-video",
        action="store_true",
        help="Reprocess video frames even if already exist",
    )
    parser.add_argument(
        "datasets",
        nargs="*",
        choices=available_datasets,
        help="Which datasets to download",
    )

    args = parser.parse_args()

    if cv2.getVersionMajor() == 4 and cv2.getVersionMinor() == 6:
        parser.error(
            """Opencv 4.6 contains a bug which will unpack some videos with the incorrect orientation.
            See: https://github.com/opencv/opencv/issues/22088
            Please upgrade or downgrade as appropriate."""
        )

    if not args.datasets:
        args.datasets = available_datasets

    for dataset in args.datasets:
        download_dataset(dataset, args.force_reprocess_video)
