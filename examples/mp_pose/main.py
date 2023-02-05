#!/usr/bin/env python3
"""Use the MediaPipe Pose solution to detect and track a human pose in video."""
import argparse
import logging
import os
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Iterator, List, Optional, Tuple

import cv2 as cv
import mediapipe as mp
import numpy as np
import numpy.typing as npt
import requests
from rerun.log.annotation import AnnotationInfo, ClassDescription

import rerun as rr

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "pose_movement"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/pose_movement"


def track_pose(video_path: str, segment: bool) -> None:
    mp_pose = mp.solutions.pose

    rr.log_annotation_context(
        "/",
        ClassDescription(
            info=AnnotationInfo(label="Person"),
            keypoint_annotations=[AnnotationInfo(id=l.value, label=l.name) for l in mp_pose.PoseLandmark],
            keypoint_connections=mp_pose.POSE_CONNECTIONS,
        ),
    )
    # Use a separate annotation context for the segmentation mask.
    rr.log_annotation_context(
        "video/mask",
        [
            AnnotationInfo(id=0, label="Background", color=(0, 0, 0)),
            AnnotationInfo(id=1, label="Person", color=(167, 80, 76)),
        ],
    )
    rr.log_view_coordinates("person", up="-Y", timeless=True)

    with closing(VideoSource(video_path)) as video_source:
        with mp_pose.Pose(enable_segmentation=segment) as pose:
            for bgr_frame in video_source.stream_bgr():
                if bgr_frame.idx < 20:
                    continue

                rgb = cv.cvtColor(bgr_frame.data, cv.COLOR_BGR2RGB)
                rr.set_time_seconds("time", bgr_frame.time)
                rr.set_time_sequence("frame_idx", bgr_frame.idx - 20)
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
        normalized_landmarks = [results.pose_landmarks.landmark[l] for l in mp.solutions.pose.PoseLandmark]
        # Log points as 3d points with some scaling so they "pop out" when looked at in a 3d view
        # Negative depth in order to move them towards the camera.
        return np.array([(image_width * l.x, image_height * l.y, -(l.z + 1.0) * 300.0) for l in normalized_landmarks])


def read_landmark_positions_3d(
    results: Any,
) -> Optional[npt.NDArray[np.float32]]:
    if results.pose_landmarks is None:
        return None
    else:
        landmarks = [results.pose_world_landmarks.landmark[l] for l in mp.solutions.pose.PoseLandmark]
        return np.array([(l.x, l.y, l.z) for l in landmarks])


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
    parser = argparse.ArgumentParser(description="Uses the MediaPipe Pose solution to track a human pose in video.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--video",
        type=str,
        default="bike",
        choices=[
            "bike",
            "backflip",
            "soccer",
        ],
        help="The example video to run on.",
    )
    parser.add_argument("--dataset_dir", type=Path, default=DATASET_DIR, help="Directory to save example videos to.")
    parser.add_argument("--video_path", type=str, default="", help="Full path to video to run on. Overrides `--video`.")
    parser.add_argument("--no-segment", action="store_true", help="Don't run person segmentation.")

    rr.init("mp_pose")

    args = parser.parse_args()

    video_path = args.video_path  # type: str
    if not video_path:
        video_path = get_downloaded_path(args.dataset_dir, args.video)

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    track_pose(video_path, segment=not args.no_segment)

    if args.save is not None:
        rr.save(args.save)


if __name__ == "__main__":
    main()
