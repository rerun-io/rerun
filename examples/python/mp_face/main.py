#!/usr/bin/env python3
"""Use the MediaPipe Face detection and Face landmark detection solutions to track human faces in images and videos."""
from __future__ import annotations

import argparse
import itertools
import logging
import math
import os
from pathlib import Path
from typing import Final

import cv2
import mediapipe as mp
import tqdm
import requests
from mediapipe.tasks.python import vision
import numpy.typing as npt

import rerun as rr  # pip install rerun-sdk

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
MODEL_DIR: Final = EXAMPLE_DIR / "model"

SAMPLE_IMAGE_PATH = (DATASET_DIR / "image.jpg").resolve()
# from https://pixabay.com/photos/brother-sister-girl-family-boy-977170/
SAMPLE_IMAGE_URL = "https://i.imgur.com/Vu2Nqwb.jpg"


def download_file(url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    logging.info("Downloading %s to %s", url, path)
    response = requests.get(url)
    response.raise_for_status()
    with open(path, "wb") as f:
        f.write(response.content)


class FaceDetectorLogger:
    MODEL_PATH: Final = (MODEL_DIR / "blaze_face_short_range.tflite").resolve()
    MODEL_URL: Final = (
        "https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/latest/"
        "blaze_face_short_range.tflite"
    )

    def __init__(self, video_mode: bool = False):
        self._video_mode = video_mode

        # download model if necessary
        if not self.MODEL_PATH.exists():
            download_file(self.MODEL_URL, self.MODEL_PATH)

        self._base_options = mp.tasks.BaseOptions(
            model_asset_path=str(self.MODEL_PATH),
        )
        self._options = vision.FaceDetectorOptions(
            base_options=self._base_options,
            running_mode=mp.tasks.vision.RunningMode.VIDEO if self._video_mode else mp.tasks.vision.RunningMode.IMAGE,
        )
        self._detector = vision.FaceDetector.create_from_options(self._options)

        rr.log_annotation_context(
            "/",
            rr.ClassDescription(keypoint_connections=[(0, 1), (1, 2), (2, 0), (2, 3), (0, 4), (1, 5)]),
        )

    def log_frame(self, image, frame_idx: int | None = None, frame_time_nano: int | None = None):
        if frame_idx is not None:
            rr.set_time_sequence("frame_nr", frame_idx)
        if frame_time_nano is not None:
            rr.set_time_nanos("frame_time", frame_time_nano)

        height, width, _ = image.shape
        image = mp.Image(image_format=mp.ImageFormat.SRGB, data=image)

        detection_result = (
            self._detector.detect_for_video(image, int(frame_time_nano / 1e6))
            if self._video_mode
            else self._detector.detect(image)
        )
        rr.log_cleared("video/faces", recursive=True)
        for i, detection in enumerate(detection_result.detections):
            # log bounding box
            bbox = detection.bounding_box
            index, score = detection.categories[0].index, detection.categories[0].score

            # log bounding box
            rr.log_rect(f"video/faces/{i}/bbox", [bbox.origin_x, bbox.origin_y, bbox.width, bbox.height])
            rr.log_extension_components(f"video/faces/{i}/bbox", {"index": index, "score": score})

            # log keypoints
            pts = [
                (math.floor(keypoint.x * width), math.floor(keypoint.y * height)) for keypoint in detection.keypoints
            ]
            rr.log_points(f"video/faces/{i}/keypoints", pts, radii=3, keypoint_ids=list(range(6)))

        rr.log_image("video/image", image.numpy_view())


def resize_image(image: npt.NDArray, max_dim: int | None) -> npt.NDArray:
    """Resize an image if it is larger than max_dim."""
    if max_dim is None:
        return image
    height, width, _ = image.shape
    scale = max_dim / max(height, width)
    if scale < 1:
        image = cv2.resize(image, (0, 0), fx=scale, fy=scale)
    return image


def run_from_video_capture(vid: int | str, max_dim: int | None, max_frame_count: int | None) -> None:
    """Run the face detector on a video stream.

    Args:
        vid: The video stream to run the detector on. Use 0 for the default camera or a path to a video file.
        max_dim: The maximum dimension of the image. If the image is larger, it will be scaled down.
        max_frame_count: The maximum number of frames to process. If None, process all frames.
    """
    cap = cv2.VideoCapture(vid)
    fps = cap.get(cv2.CAP_PROP_FPS)

    logger = FaceDetectorLogger(video_mode=True)

    print("Capturing video stream. Press ctrl-c to stop.")
    try:
        if max_frame_count is not None:
            it = range(max_frame_count)
        else:
            it = itertools.count()

        for frame_idx in tqdm.tqdm(it):
            # Capture frame-by-frame
            ret, frame = cap.read()
            if not ret:
                break

            frame = resize_image(frame, max_dim)

            # get frame time
            frame_time_nano = int(cap.get(cv2.CAP_PROP_POS_MSEC) * 1e6)
            if frame_time_nano == 0:
                # On some platforms it always returns zero, so we compute from the frame counter and fps
                frame_time_nano = int(frame_idx * 1000 / fps * 1e6)

            # convert to rgb
            frame = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
            logger.log_frame(frame, frame_idx=frame_idx, frame_time_nano=frame_time_nano)

    except KeyboardInterrupt:
        pass

    # When everything done, release the capture
    cap.release()
    cv2.destroyAllWindows()


def run_from_sample_image(path: Path, max_dim: int | None) -> None:
    """Run the face detector on a single image."""
    image = cv2.imread(str(path))
    image = resize_image(image, max_dim)
    image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    logger = FaceDetectorLogger(video_mode=False)
    logger.log_frame(image)


def main() -> None:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Uses the MediaPipe Face Detection to track a human pose in video.")
    parser.add_argument(
        "--demo-image",
        action="store_true",
        help="Run on a demo image automatically downloaded",
    )
    parser.add_argument(
        "--image",
        type=Path,
        help="Run on the provided image",
    )
    parser.add_argument("--video", type=Path, help="Run on the provided video file.")
    parser.add_argument(
        "--camera", type=int, default=0, help="Run from the camera stream (parameter is the camera ID, usually 0"
    )
    parser.add_argument(
        "--max-frame",
        type=int,
        help="Stop after processing this many frames. If not specified, will run until interrupted.",
    )
    parser.add_argument(
        "--max-dim",
        type=int,
        help="Resize the image such as its maximum dimension is not larger than this value.",
    )

    rr.script_add_args(parser)

    args, unknown = parser.parse_known_args()
    [logging.warning(f"unknown arg: {arg}") for arg in unknown]
    rr.script_setup(args, "mp_face_detection")

    if args.demo_image:
        if not SAMPLE_IMAGE_PATH.exists():
            download_file(SAMPLE_IMAGE_URL, SAMPLE_IMAGE_PATH)

        run_from_sample_image(SAMPLE_IMAGE_PATH, args.max_dim)
    elif args.image is not None:
        run_from_sample_image(args.image, args.max_dim)
    elif args.video is not None:
        run_from_video_capture(str(args.video), args.max_dim, args.max_frame)
    else:
        run_from_video_capture(args.camera, args.max_dim, args.max_frame)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
