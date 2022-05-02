#!/usr/bin/env python3

# TODO: translate to build.rs instead?

import requests
import os


def download(url, path):
    if not os.path.exists(path):
        print(f"downloading {url}…")
        response = requests.get(url)
        with open(path, "wb") as file:
            file.write(response.content)


def split_video_into_frames(video_path, frames_path):
    if not os.path.exists(frames_path):
        print("Splitting video into frames…")
        os.makedirs(frames_path, exist_ok=True)

        import cv2
        vidcap = cv2.VideoCapture(video_path)
        success, image = vidcap.read()
        count = 0
        while success:
            cv2.imwrite(f"{frames_path}/{count}.jpg", image)
            success, image = vidcap.read()
            count += 1


public_url = "https://storage.googleapis.com/objectron"


def download_data(video_id):
    print(f"downloading {video_id}…")

    dir = f"dataset/{video_id}"
    os.makedirs(dir, exist_ok=True)

    download(f"{public_url}/videos/{video_id}/video.MOV",
             f"{dir}/video.MOV")

    # use object.proto
    download(f"{public_url}/videos/{video_id}/geometry.pbdata",
             f"{dir}/geometry.pbdata")

    # Please refer to Parse Annotation tutorial to see how to parse the annotation files.
    download(f"{public_url}/annotations/{video_id}.pbdata",
             f"{dir}/annotation.pbdata")

    split_video_into_frames(f"{dir}/video.MOV", f"{dir}/video")


def download_dataset(name):
    video_ids = requests.get(
        f"{public_url}/v1/index/{name}_annotations_test").text
    video_ids = video_ids.split('\n')
    for i in range(10):
        download_data(video_ids[i])


download_dataset("bike")
download_dataset("book")
download_dataset("bottle")
download_dataset("camera")
download_dataset("cereal_box")
download_dataset("chair")
download_dataset("cup")
download_dataset("laptop")
download_dataset("shoe")
