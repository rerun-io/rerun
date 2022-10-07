#!/usr/bin/env python3

import argparse
import requests
import os
import cv2

import requests


def download(url, path):
    if not os.path.exists(path):
        print(f"downloading {url}…")
        response = requests.get(url)
        with open(path, "wb") as file:
            file.write(response.content)


def split_video_into_frames(video_path, frames_path, reprocess_video):
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


public_url = "https://storage.googleapis.com/objectron"


def download_data(video_id, reprocess_video):
    print(f"downloading {video_id}…")

    dir = f"dataset/{video_id}"
    os.makedirs(dir, exist_ok=True)

    download(f"{public_url}/videos/{video_id}/video.MOV", f"{dir}/video.MOV")

    # use object.proto
    download(
        f"{public_url}/videos/{video_id}/geometry.pbdata", f"{dir}/geometry.pbdata"
    )

    # Please refer to Parse Annotation tutorial to see how to parse the annotation files.
    download(f"{public_url}/annotations/{video_id}.pbdata", f"{dir}/annotation.pbdata")

    split_video_into_frames(f"{dir}/video.MOV", f"{dir}/video", reprocess_video)


def download_dataset(name, reprocess_video):
    video_ids = requests.get(f"{public_url}/v1/index/{name}_annotations_test").text
    video_ids = video_ids.split("\n")
    for i in range(3):
        download_data(video_ids[i], reprocess_video)


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
        choices=available_datasets + [[]],
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
