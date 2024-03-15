#!/usr/bin/env python3
"""Use the MediaPipe Pose solution to detect and track a human pose in video."""
from __future__ import annotations

import argparse
import logging
import os
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Iterator

import cv2
import mediapipe as mp
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "pose_movement"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/pose_movement"


DESCRIPTION = """
# Human Pose Tracking
This example uses Rerun to visualize the output of [MediaPipe](https://developers.google.com/mediapipe)-based tracking
of a human pose in 2D and 3D.

## How it was made
The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/human_pose_tracking/main.py).

### Input Video
The input video is logged as a sequence of
[rr.Image objects](https://www.rerun.io/docs/reference/types/archetypes/image) to the [video entity](recording://video).

### Segmentation
The [segmentation result](recording://video/mask) is logged through a combination of two archetypes. The segmentation
image itself is logged as an
[rr.SegmentationImage archetype](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) and
contains the id for each pixel. The color is determined by the
[rr.AnnotationContext archetype](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) which is
logged with `rr.log(…, static=True` as it should apply to the whole sequence.

### Skeletons
The [2D](recording://video/pose/points) and [3D skeletons](recording://person/pose/points) are also logged through a
similar combination of two entities.

First, a timeless
[rr.ClassDescription](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged (note, that
this is equivalent to logging an
[rr.AnnotationContext archetype](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) as in the
segmentation case). The class description contains the information which maps keypoint ids to labels and how to connect
the keypoints to a skeleton.

Second, the actual keypoint positions are logged in 2D
nd 3D as [rr.Points2D](https://www.rerun.io/docs/reference/types/archetypes/points2d) and
[rr.Points3D](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetypes, respectively.
""".strip()


def track_pose(video_path: str, *, segment: bool, max_frame_count: int | None) -> None:
    mp_pose = mp.solutions.pose

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    rr.log(
        "/",
        rr.AnnotationContext(
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=1, label="Person"),
                keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
                keypoint_connections=mp_pose.POSE_CONNECTIONS,
            )
        ),
        static=True,
    )
    # Use a separate annotation context for the segmentation mask.
    rr.log(
        "video/mask",
        rr.AnnotationContext(
            [
                rr.AnnotationInfo(id=0, label="Background"),
                rr.AnnotationInfo(id=1, label="Person", color=(0, 0, 0)),
            ]
        ),
        static=True,
    )
    rr.log("person", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, static=True)

    with closing(VideoSource(video_path)) as video_source, mp_pose.Pose(enable_segmentation=segment) as pose:
        for idx, bgr_frame in enumerate(video_source.stream_bgr()):
            if max_frame_count is not None and idx >= max_frame_count:
                break

            rgb = cv2.cvtColor(bgr_frame.data, cv2.COLOR_BGR2RGB)
            rr.set_time_seconds("time", bgr_frame.time)
            rr.set_time_sequence("frame_idx", bgr_frame.idx)
            rr.log("video/rgb", rr.Image(rgb).compress(jpeg_quality=75))

            results = pose.process(rgb)
            h, w, _ = rgb.shape
            landmark_positions_2d = read_landmark_positions_2d(results, w, h)
            if landmark_positions_2d is not None:
                rr.log(
                    "video/pose/points",
                    rr.Points2D(landmark_positions_2d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark),
                )

            landmark_positions_3d = read_landmark_positions_3d(results)
            if landmark_positions_3d is not None:
                rr.log(
                    "person/pose/points",
                    rr.Points3D(landmark_positions_3d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark),
                )

            segmentation_mask = results.segmentation_mask
            if segmentation_mask is not None:
                rr.log("video/mask", rr.SegmentationImage(segmentation_mask.astype(np.uint8)))


def read_landmark_positions_2d(
    results: Any,
    image_width: int,
    image_height: int,
) -> npt.NDArray[np.float32] | None:
    if results.pose_landmarks is None:
        return None
    else:
        normalized_landmarks = [results.pose_landmarks.landmark[lm] for lm in mp.solutions.pose.PoseLandmark]
        return np.array([(image_width * lm.x, image_height * lm.y) for lm in normalized_landmarks])


def read_landmark_positions_3d(
    results: Any,
) -> npt.NDArray[np.float32] | None:
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
        self.capture = cv2.VideoCapture(path)

        if not self.capture.isOpened():
            logging.error("Couldn't open video at %s", path)

    def close(self) -> None:
        self.capture.release()

    def stream_bgr(self) -> Iterator[VideoFrame]:
        while self.capture.isOpened():
            idx = int(self.capture.get(cv2.CAP_PROP_POS_FRAMES))
            is_open, bgr = self.capture.read()
            time_ms = self.capture.get(cv2.CAP_PROP_POS_MSEC)

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
    parser.add_argument("--dataset-dir", type=Path, default=DATASET_DIR, help="Directory to save example videos to.")
    parser.add_argument("--video-path", type=str, default="", help="Full path to video to run on. Overrides `--video`.")
    parser.add_argument("--no-segment", action="store_true", help="Don't run person segmentation.")
    parser.add_argument(
        "--max-frame",
        type=int,
        help="Stop after processing this many frames. If not specified, will run until interrupted.",
    )
    rr.script_add_args(parser)

    args = parser.parse_args()
    rr.script_setup(args, "rerun_example_human_pose_tracking")

    video_path = args.video_path  # type: str
    if not video_path:
        video_path = get_downloaded_path(args.dataset_dir, args.video)

    track_pose(video_path, segment=not args.no_segment, max_frame_count=args.max_frame)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
