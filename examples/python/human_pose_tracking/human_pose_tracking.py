#!/usr/bin/env python3
"""Use the MediaPipe Pose solution to detect and track a human pose in video."""

from __future__ import annotations

import argparse
import logging
import os
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, Final

import cv2
import mediapipe as mp
import mediapipe.python.solutions.pose as mp_pose
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from collections.abc import Iterator

DESCRIPTION = """
# Human pose tracking
This example uses Rerun to visualize the output of [MediaPipe](https://developers.google.com/mediapipe)-based tracking
of a human pose in 2D and 3D.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/human_pose_tracking).
""".strip()

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "pose_movement"
MODEL_DIR: Final = EXAMPLE_DIR / "model" / "pose_movement"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/pose_movement"
MODEL_URL_TEMPLATE: Final = "https://storage.googleapis.com/mediapipe-models/pose_landmarker/pose_landmarker_{model_name}/float16/latest/pose_landmarker_{model_name}.task"


def track_pose(video_path: str, model_path: str, *, max_frame_count: int | None) -> None:
    options = mp.tasks.vision.PoseLandmarkerOptions(
        base_options=mp.tasks.BaseOptions(
            model_asset_path=model_path,
        ),
        running_mode=mp.tasks.vision.RunningMode.VIDEO,
        output_segmentation_masks=True,
    )

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    rr.log(
        "/",
        rr.AnnotationContext(
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=1, label="Person"),
                keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
                keypoint_connections=mp_pose.POSE_CONNECTIONS,
            ),
        ),
        static=True,
    )
    # Use a separate annotation context for the segmentation mask.
    rr.log(
        "video/mask",
        rr.AnnotationContext([
            rr.AnnotationInfo(id=0, label="Background"),
            rr.AnnotationInfo(id=1, label="Person", color=(0, 0, 0)),
        ]),
        static=True,
    )
    rr.log("person", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, static=True)

    pose_landmarker = mp.tasks.vision.PoseLandmarker.create_from_options(options)

    with closing(VideoSource(video_path)) as video_source:
        for idx, bgr_frame in enumerate(video_source.stream_bgr()):
            if max_frame_count is not None and idx >= max_frame_count:
                break

            mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=bgr_frame.data)
            rr.set_time("time", duration=bgr_frame.time)
            rr.set_time("frame_idx", sequence=bgr_frame.idx)

            results = pose_landmarker.detect_for_video(mp_image, int(bgr_frame.time * 1000))
            h, w, _ = bgr_frame.data.shape
            landmark_positions_2d = read_landmark_positions_2d(results, w, h)

            rr.log("video/bgr", rr.Image(bgr_frame.data, color_model="BGR").compress(jpeg_quality=75))
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

            if results.segmentation_masks is not None:
                segmentation_mask = results.segmentation_masks[0].numpy_view()
                binary_segmentation_mask = segmentation_mask > 0.5
                rr.log("video/mask", rr.SegmentationImage(binary_segmentation_mask.astype(np.uint8)))


def read_landmark_positions_2d(
    results: Any,
    image_width: int,
    image_height: int,
) -> npt.NDArray[np.float32] | None:
    if results.pose_landmarks is None or len(results.pose_landmarks) == 0:
        return None
    else:
        pose_landmarks = results.pose_landmarks[0]
        normalized_landmarks = [pose_landmarks[lm] for lm in mp_pose.PoseLandmark]
        return np.array([(image_width * lm.x, image_height * lm.y) for lm in normalized_landmarks])


def read_landmark_positions_3d(
    results: Any,
) -> npt.NDArray[np.float32] | None:
    if results.pose_landmarks is None or len(results.pose_landmarks) == 0:
        return None
    else:
        pose_landmarks = results.pose_landmarks[0]
        landmarks = [pose_landmarks[lm] for lm in mp_pose.PoseLandmark]
        return np.array([(lm.x, lm.y, lm.z) for lm in landmarks])


@dataclass
class VideoFrame:
    data: cv2.typing.MatLike
    time: float
    idx: int


class VideoSource:
    def __init__(self, path: str) -> None:
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


def get_downloaded_video_path(dataset_dir: Path, video_name: str) -> str:
    video_file_name = f"{video_name}.mp4"
    destination_path = dataset_dir / video_file_name
    if destination_path.exists():
        logging.info("%s already exists. No need to download", destination_path)
        return str(destination_path)

    source_path = f"{DATASET_URL_BASE}/{video_file_name}"

    logging.info("Downloading video from %s to %s", source_path, destination_path)
    os.makedirs(dataset_dir.absolute(), exist_ok=True)
    download(source_path, destination_path)
    return str(destination_path)


def get_downloaded_model_path(model_dir: Path, model_name: str) -> str:
    model_file_name = f"{model_name}.task"
    destination_path = model_dir / model_file_name
    if destination_path.exists():
        logging.info("%s already exists. No need to download", destination_path)
        return str(destination_path)

    model_url = MODEL_URL_TEMPLATE.format(model_name=model_name)
    logging.info("Downloading model from %s to %s", model_url, destination_path)
    download(model_url, destination_path)
    return str(destination_path)


def download(url: str, destination_path: Path) -> None:
    os.makedirs(destination_path.parent, exist_ok=True)
    with requests.get(url, stream=True) as req:
        req.raise_for_status()
        with open(destination_path, "wb") as f:
            f.writelines(req.iter_content(chunk_size=8192))


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
    parser.add_argument(
        "--model",
        type=str,
        default="heavy",
        choices=["lite", "full", "heavy"],
        help="The mediapipe model to use (see https://developers.google.com/mediapipe/solutions/vision/pose_landmarker).",
    )
    parser.add_argument("--model-dir", type=Path, default=MODEL_DIR, help="Directory to save downloaded model to.")
    parser.add_argument("--model-path", type=str, default="", help="Full path of mediapipe model. Overrides `--model`.")
    parser.add_argument(
        "--max-frame",
        type=int,
        help="Stop after processing this many frames. If not specified, will run until interrupted.",
    )
    rr.script_add_args(parser)

    args = parser.parse_args()
    rr.script_setup(
        args,
        "rerun_example_human_pose_tracking",
        default_blueprint=rrb.Horizontal(
            rrb.Vertical(
                rrb.Spatial2DView(origin="video", name="Result"),
                rrb.Spatial3DView(origin="person", name="3D pose"),
            ),
            rrb.Vertical(
                rrb.Spatial2DView(origin="video/bgr", name="Raw video"),
                rrb.TextDocumentView(origin="description", name="Description"),
                row_shares=[2, 3],
            ),
            column_shares=[3, 2],
        ),
    )

    video_path = args.video_path  # type: str
    if not video_path:
        video_path = get_downloaded_video_path(args.dataset_dir, args.video)

    model_path = args.model_path  # type: str
    if not args.model_path:
        model_path = get_downloaded_model_path(args.model_dir, args.model)

    track_pose(video_path, model_path, max_frame_count=args.max_frame)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
