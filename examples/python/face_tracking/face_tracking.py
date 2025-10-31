#!/usr/bin/env python3
"""Use the MediaPipe Face detection and Face landmark detection solutions to track human faces in images and videos."""

from __future__ import annotations

import argparse
import itertools
import logging
import math
import os
from pathlib import Path
from typing import TYPE_CHECKING, Any, Final

import cv2
import mediapipe as mp
import numpy as np
import requests
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
import tqdm
from mediapipe.tasks.python import vision

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator

# If set, log everything as static.
#
# Generally, the Viewer accumulates data until its set memory budget at which point it will
# remove the oldest data from the recording (see https://rerun.io/docs/howto/limit-ram)
# By instead logging data as static, no data will be accumulated over time since previous
# data is overwritten.
# Naturally, the drawback of this is that there's no history of previous data sent to the viewer,
# as well as no timestamps, making the Viewer's timeline effectively inactive.
global ALL_STATIC
ALL_STATIC: bool = False

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
MODEL_DIR: Final = EXAMPLE_DIR / "model"

SAMPLE_IMAGE_PATH = (DATASET_DIR / "image.jpg").resolve()
# from https://pixabay.com/photos/brother-sister-girl-family-boy-977170/
SAMPLE_IMAGE_URL = "https://i.imgur.com/Vu2Nqwb.jpg"

# uncomment blendshapes of interest
BLENDSHAPES_CATEGORIES = {
    "_neutral",
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "eyeBlinkLeft",
    "eyeBlinkRight",
    "eyeLookDownLeft",
    "eyeLookDownRight",
    "eyeLookInLeft",
    "eyeLookInRight",
    "eyeLookOutLeft",
    "eyeLookOutRight",
    "eyeLookUpLeft",
    "eyeLookUpRight",
    "eyeSquintLeft",
    "eyeSquintRight",
    "eyeWideLeft",
    "eyeWideRight",
    "jawForward",
    "jawLeft",
    "jawOpen",
    "jawRight",
    "mouthClose",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthFunnel",
    "mouthLeft",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthPucker",
    "mouthRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "noseSneerLeft",
    "noseSneerRight",
}


class FaceDetectorLogger:
    """
    Logger for the MediaPipe Face Detection solution.

    <https://developers.google.com/mediapipe/solutions/vision/face_detector>
    """

    MODEL_PATH: Final = (MODEL_DIR / "blaze_face_short_range.tflite").resolve()
    MODEL_URL: Final = (
        "https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/latest/"
        "blaze_face_short_range.tflite"
    )

    def __init__(self, video_mode: bool = False) -> None:
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

        # With this annotation, the viewer will connect the keypoints with some lines to improve visibility.
        rr.log(
            "video/detector",
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=0),
                keypoint_connections=[(0, 1), (1, 2), (2, 0), (2, 3), (0, 4), (1, 5)],
            ),
            static=True,
        )

    def detect_and_log(self, image: cv2.typing.MatLike, frame_time_nano: int) -> None:
        height, width, _ = image.shape
        image = mp.Image(image_format=mp.ImageFormat.SRGB, data=image)

        detection_result = (
            self._detector.detect_for_video(image, int(frame_time_nano / 1e6))
            if self._video_mode
            else self._detector.detect(image)
        )
        rr.log("video/detector/faces", rr.Clear(recursive=True), static=ALL_STATIC)
        for i, detection in enumerate(detection_result.detections):
            # log bounding box
            bbox = detection.bounding_box
            index, score = detection.categories[0].index, detection.categories[0].score

            # log bounding box
            rr.log(
                f"video/detector/faces/{i}/bbox",
                rr.Boxes2D(
                    array=[bbox.origin_x, bbox.origin_y, bbox.width, bbox.height],
                    array_format=rr.Box2DFormat.XYWH,
                ),
                rr.AnyValues(index=index, score=score),
                static=ALL_STATIC,
            )

            # MediaPipe's keypoints are normalized to [0, 1], so we need to scale them to get pixel coordinates.
            pts = [
                (math.floor(keypoint.x * width), math.floor(keypoint.y * height)) for keypoint in detection.keypoints
            ]
            rr.log(
                f"video/detector/faces/{i}/keypoints",
                rr.Points2D(pts, radii=3, keypoint_ids=list(range(6))),
                static=ALL_STATIC,
            )


class FaceLandmarkerLogger:
    """
    Logger for the MediaPipe Face Landmark Detection solution.

    <https://developers.google.com/mediapipe/solutions/vision/face_landmarker>
    """

    MODEL_PATH: Final = (MODEL_DIR / "face_landmarker.task").resolve()
    MODEL_URL: Final = (
        "https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/latest/"
        "face_landmarker.task"
    )

    def __init__(self, video_mode: bool = False, num_faces: int = 1) -> None:
        self._video_mode = video_mode

        # download model if necessary
        if not self.MODEL_PATH.exists():
            download_file(self.MODEL_URL, self.MODEL_PATH)

        self._base_options = mp.tasks.BaseOptions(
            model_asset_path=str(self.MODEL_PATH),
        )
        self._options = vision.FaceLandmarkerOptions(
            base_options=self._base_options,
            output_face_blendshapes=True,
            num_faces=num_faces,
            running_mode=mp.tasks.vision.RunningMode.VIDEO if self._video_mode else mp.tasks.vision.RunningMode.IMAGE,
        )
        self._detector = vision.FaceLandmarker.create_from_options(self._options)

        # Extract classes from MediaPipe face mesh solution. The goal of this code is:
        # 1) Log an annotation context with one class ID per facial feature. For each class ID, the class description
        #    contains the connections between corresponding keypoints (taken from the MediaPipe face mesh solution)
        # 2) A class ID array matching the class IDs in the annotation context to keypoint indices (to be passed as
        #    the `class_ids` argument to `rr.log`).

        classes = [
            mp.solutions.face_mesh.FACEMESH_LIPS,
            mp.solutions.face_mesh.FACEMESH_LEFT_EYE,
            mp.solutions.face_mesh.FACEMESH_LEFT_IRIS,
            mp.solutions.face_mesh.FACEMESH_LEFT_EYEBROW,
            mp.solutions.face_mesh.FACEMESH_RIGHT_EYE,
            mp.solutions.face_mesh.FACEMESH_RIGHT_EYEBROW,
            mp.solutions.face_mesh.FACEMESH_RIGHT_IRIS,
            mp.solutions.face_mesh.FACEMESH_FACE_OVAL,
            mp.solutions.face_mesh.FACEMESH_NOSE,
        ]

        self._class_ids = [0] * mp.solutions.face_mesh.FACEMESH_NUM_LANDMARKS_WITH_IRISES
        class_descriptions = []
        for i, klass in enumerate(classes):
            # MediaPipe only provides connections for class, not actual class per keypoint. So we have to extract the
            # classes from the connections.
            ids = set()
            for connection in klass:
                ids.add(connection[0])
                ids.add(connection[1])

            for id_ in ids:
                self._class_ids[id_] = i

            class_descriptions.append(
                rr.ClassDescription(
                    info=rr.AnnotationInfo(id=i),
                    keypoint_connections=klass,
                ),
            )

        rr.log("video/landmarker", rr.AnnotationContext(class_descriptions), static=True)
        rr.log("reconstruction", rr.AnnotationContext(class_descriptions), static=True)

        # properly align the 3D face in the viewer
        rr.log("reconstruction", rr.ViewCoordinates.RDF, static=True)

    def detect_and_log(self, image: cv2.typing.MatLike, frame_time_nano: int) -> None:
        height, width, _ = image.shape
        image = mp.Image(image_format=mp.ImageFormat.SRGB, data=image)

        detection_result = (
            self._detector.detect_for_video(image, int(frame_time_nano / 1e6))
            if self._video_mode
            else self._detector.detect(image)
        )

        def is_empty(i: Iterator[Any]) -> bool:
            try:
                next(i)
                return False
            except StopIteration:
                return True

        if is_empty(zip(detection_result.face_landmarks, detection_result.face_blendshapes, strict=False)):
            rr.log("video/landmarker/faces", rr.Clear(recursive=True), static=ALL_STATIC)
            rr.log("reconstruction/faces", rr.Clear(recursive=True), static=ALL_STATIC)
            rr.log("blendshapes", rr.Clear(recursive=True), static=ALL_STATIC)

        for i, (landmark, blendshapes) in enumerate(
            zip(detection_result.face_landmarks, detection_result.face_blendshapes, strict=False),
        ):
            if len(landmark) == 0 or len(blendshapes) == 0:
                rr.log(
                    f"video/landmarker/faces/{i}/landmarks",
                    rr.Clear(recursive=True),
                    static=ALL_STATIC,
                )
                rr.log(
                    f"reconstruction/faces/{i}",
                    rr.Clear(recursive=True),
                    static=ALL_STATIC,
                )
                rr.log(f"blendshapes/{i}", rr.Clear(recursive=True), static=ALL_STATIC)
                continue

            # MediaPipe's keypoints are normalized to [0, 1], so we need to scale them to get pixel coordinates.
            pts = [(math.floor(lm.x * width), math.floor(lm.y * height)) for lm in landmark]
            keypoint_ids = list(range(len(landmark)))
            rr.log(
                f"video/landmarker/faces/{i}/landmarks",
                rr.Points2D(pts, radii=3, keypoint_ids=keypoint_ids, class_ids=self._class_ids),
                static=ALL_STATIC,
            )

            rr.log(
                f"reconstruction/faces/{i}",
                rr.Points3D(
                    [(lm.x, lm.y, lm.z) for lm in landmark],
                    keypoint_ids=keypoint_ids,
                    class_ids=self._class_ids,
                ),
                static=ALL_STATIC,
            )

            for blendshape in blendshapes:
                if blendshape.category_name in BLENDSHAPES_CATEGORIES:
                    # NOTE(cmc): That one we still log as temporal, otherwise it's really meh.
                    rr.log(
                        f"blendshapes/{i}/{blendshape.category_name}",
                        rr.Scalars(blendshape.score),
                    )


# ========================================================================================
# Main & CLI code


def download_file(url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    logging.info("Downloading %s to %s", url, path)
    response = requests.get(url, stream=True)
    with tqdm.tqdm.wrapattr(
        open(path, "wb"),
        "write",
        miniters=1,
        total=int(response.headers.get("content-length", 0)),
        desc=f"Downloading {path.name}",
    ) as f:
        for chunk in response.iter_content(chunk_size=4096):
            f.write(chunk)


def resize_image(image: cv2.typing.MatLike, max_dim: int | None) -> cv2.typing.MatLike:
    """Resize an image if it is larger than max_dim."""
    if max_dim is None:
        return image
    height, width, _ = image.shape
    scale = max_dim / max(height, width)
    if scale < 1:
        image = cv2.resize(image, (0, 0), fx=scale, fy=scale)
    return image


def run_from_video_capture(vid: int | str, max_dim: int | None, max_frame_count: int | None, num_faces: int) -> None:
    """
    Run the face detector on a video stream.

    Parameters
    ----------
    vid:
        The video stream to run the detector on. Use 0 for the default camera or a path to a video file.
    max_dim:
        The maximum dimension of the image. If the image is larger, it will be scaled down.
    max_frame_count:
        The maximum number of frames to process. If None, process all frames.
    num_faces:
        The number of faces to track. If set to 1, temporal smoothing will be applied.

    """

    cap = cv2.VideoCapture(vid)
    fps = cap.get(cv2.CAP_PROP_FPS)

    detector = FaceDetectorLogger(video_mode=True)
    landmarker = FaceLandmarkerLogger(video_mode=True, num_faces=num_faces)

    print("Capturing video stream. Press ctrl-c to stop.")
    try:
        it: Iterable[int] = itertools.count() if max_frame_count is None else range(max_frame_count)

        for frame_idx in tqdm.tqdm(it, desc="Processing frames"):
            # Capture frame-by-frame
            ret, frame = cap.read()
            if not ret:
                break

            # OpenCV sometimes returns a blank frame, so we skip it
            if np.all(frame == 0):
                continue

            frame = resize_image(frame, max_dim)

            # get frame time
            frame_time_nano = int(cap.get(cv2.CAP_PROP_POS_MSEC) * 1e6)
            if frame_time_nano == 0:
                # On some platforms it always returns zero, so we compute from the frame counter and fps
                frame_time_nano = int(frame_idx * 1000 / fps * 1e6)

            # log data
            rr.set_time("frame_nr", sequence=frame_idx)
            rr.set_time("frame_time", duration=1e-9 * frame_time_nano)
            detector.detect_and_log(frame, frame_time_nano)
            landmarker.detect_and_log(frame, frame_time_nano)
            rr.log(
                "video/image",
                rr.Image(frame, color_model="BGR"),
                static=ALL_STATIC,
            )

    except KeyboardInterrupt:
        pass

    # When everything done, release the capture
    cap.release()
    cv2.destroyAllWindows()


def run_from_sample_image(path: Path, max_dim: int | None, num_faces: int) -> None:
    """Run the face detector on a single image."""
    image = cv2.imread(str(path))
    image = resize_image(image, max_dim)
    logger = FaceDetectorLogger(video_mode=False)
    landmarker = FaceLandmarkerLogger(video_mode=False, num_faces=num_faces)
    logger.detect_and_log(image, 0)
    landmarker.detect_and_log(image, 0)
    rr.log(
        "video/image",
        rr.Image(image, color_model="BGR"),
        static=ALL_STATIC,
    )


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
        "--camera",
        type=int,
        default=0,
        help="Run from the camera stream (parameter is the camera ID, usually 0)",
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
    parser.add_argument(
        "--num-faces",
        type=int,
        default=1,
        help=(
            "Max number of faces detected by the landmark model (temporal smoothing is applied only for a value of 1)."
        ),
    )
    parser.add_argument("--static", action="store_true", help="If set, logs everything as static")

    rr.script_add_args(parser)

    args, unknown = parser.parse_known_args()
    for arg in unknown:
        logging.warning(f"unknown arg: {arg}")

    rr.script_setup(
        args,
        "rerun_example_mp_face_detection",
        default_blueprint=rrb.Horizontal(
            rrb.Spatial3DView(origin="reconstruction"),
            rrb.Vertical(
                rrb.Spatial2DView(origin="video"),
                rrb.TimeSeriesView(
                    origin="blendshapes",
                    # Enable only certain blend shapes by default. More can be added in the viewer ui
                    contents=[
                        "+ blendshapes/0/eyeBlinkLeft",
                        "+ blendshapes/0/eyeBlinkRight",
                        "+ blendshapes/0/jawOpen",
                        "+ blendshapes/0/mouthSmileLeft",
                        "+ blendshapes/0/mouthSmileRight",
                    ],
                ),
            ),
        ),
    )

    global ALL_STATIC
    ALL_STATIC = args.static

    if args.demo_image:
        if not SAMPLE_IMAGE_PATH.exists():
            download_file(SAMPLE_IMAGE_URL, SAMPLE_IMAGE_PATH)

        run_from_sample_image(SAMPLE_IMAGE_PATH, args.max_dim, args.num_faces)
    elif args.image is not None:
        run_from_sample_image(args.image, args.max_dim, args.num_faces)
    elif args.video is not None:
        run_from_video_capture(str(args.video), args.max_dim, args.max_frame, args.num_faces)
    else:
        run_from_video_capture(args.camera, args.max_dim, args.max_frame, args.num_faces)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
