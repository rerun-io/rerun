#!/usr/bin/env python3
"""Use the MediaPipe Pose solution to detect and track a human pose in video."""
import argparse
import logging
import os
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Iterator, Optional

import cv2 as cv
import mediapipe as mp
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "pose_movement"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/pose_movement"


def track_pose(video_path: str, segment: bool) -> None:
    mp_pose = mp.solutions.pose

    rr.log_annotation_context(
        "/",
        rr.ClassDescription(
            info=rr.AnnotationInfo(label="Person"),
            keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
            keypoint_connections=mp_pose.POSE_CONNECTIONS,
        ),
    )
    # Use a separate annotation context for the segmentation mask.
    rr.log_annotation_context(
        "video/mask",
        [rr.AnnotationInfo(id=0, label="Background"), rr.AnnotationInfo(id=1, label="Person", color=(0, 0, 0))],
    )
    rr.log_view_coordinates("person", up="-Y", timeless=True)

    with closing(VideoSource(video_path)) as video_source, mp_pose.Pose(enable_segmentation=segment) as pose:
        for bgr_frame in video_source.stream_bgr():
            rgb = cv.cvtColor(bgr_frame.data, cv.COLOR_BGR2RGB)
            rr.set_time_seconds("time", bgr_frame.time)
            rr.set_time_sequence("frame_idx", bgr_frame.idx)
            rr.log_image("video/rgb", rgb)

            results = pose.process(rgb)
            h, w, _ = rgb.shape
            landmark_positions_2d = read_landmark_positions_2d(results, w, h)
            rr.log_points("video/pose/points", landmark_positions_2d, keypoint_ids=mp_pose.PoseLandmark)

            landmark_positions_3d = read_landmark_positions_3d(results)
            rr.log_points("person/pose/points", landmark_positions_3d, keypoint_ids=mp_pose.PoseLandmark)

            segmentation_mask = results.segmentation_mask
            if segmentation_mask is not None:
                rr.log_segmentation_image("video/mask", segmentation_mask)


def read_landmark_positions_2d(
    results: Any,
    image_width: int,
    image_height: int,
) -> Optional[npt.NDArray[np.float32]]:
    if results.pose_landmarks is None:
        return None
    else:
        normalized_landmarks = [results.pose_landmarks.landmark[lm] for lm in mp.solutions.pose.PoseLandmark]
        return np.array([(image_width * lm.x, image_height * lm.y) for lm in normalized_landmarks])


def read_landmark_positions_3d(
    results: Any,
) -> Optional[npt.NDArray[np.float32]]:
    if results.pose_landmarks is None:
        return None
    else:
        landmarks = [results.pose_world_landmarks.landmark[lm] for lm in mp.solutions.pose.PoseLandmark]
        return np.array([(lm.x, lm.y, lm.z) for lm in landmarks])


@dataclass
class VideoFrame:
    data: npt.NDArray[np.uint8]
    time: float
    idx: int


class VideoSource:
    def __init__(self, path: str):
        self.capture = cv.VideoCapture(path)

        if not self.capture.isOpened():
            logging.error("Couldn't open video at %s", path)

    def close(self) -> None:
        self.capture.release()

    def stream_bgr(self) -> Iterator[VideoFrame]:
        while self.capture.isOpened():
            idx = int(self.capture.get(cv.CAP_PROP_POS_FRAMES))
            is_open, bgr = self.capture.read()
            time_ms = self.capture.get(cv.CAP_PROP_POS_MSEC)

            if not is_open:
                break

            yield VideoFrame(data=bgr, time=time_ms * 1e-3, idx=idx)


def get_downloaded_path(dataset_dir: Path, video_name: str) -> str:
    video_file_name = f"{video_name}.mp4"
    destination_path = dataset_dir / video_file_name
    if destination_path.exists():
        logging.info("%s already exists. No need to download", destination_path)
        return str(destination_path)

    source_path = f"{DATASET_URL_BASE}/{video_file_name}"

    logging.info("Downloading video from %s to %s", source_path, destination_path)
    os.makedirs(dataset_dir.absolute(), exist_ok=True)
    with requests.get(source_path, stream=True) as req:
        req.raise_for_status()
        with open(destination_path, "wb") as f:
            for chunk in req.iter_content(chunk_size=8192):
                f.write(chunk)
    return str(destination_path)


def main() -> None:
    # Ensure the logging gets written to stderr:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Uses the MediaPipe Pose solution to track a human pose in video.")
    parser.add_argument(
        "--video",
        type=str,
        default="backflip",
        choices=["backflip", "soccer"],
        help="The example video to run on.",
    )
    parser.add_argument("--dataset_dir", type=Path, default=DATASET_DIR, help="Directory to save example videos to.")
    parser.add_argument("--video_path", type=str, default="", help="Full path to video to run on. Overrides `--video`.")
    parser.add_argument("--no-segment", action="store_true", help="Don't run person segmentation.")
    rr.script_add_args(parser)

    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]
    rr.script_setup(args, "mp_pose")

    video_path = args.video_path  # type: str
    if not video_path:
        video_path = get_downloaded_path(args.dataset_dir, args.video)

    track_pose(video_path, segment=not args.no_segment)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
