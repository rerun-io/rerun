"""Use the MediaPipe Gesture detection and Gesture landmark detection solutions to track hands and recognise gestures
in images and videos."""
from __future__ import annotations

import mediapipe as mp
from mediapipe.tasks.python import vision
from mediapipe.tasks import python
from pathlib import Path
from typing import Final, Iterable, Any

import rerun as rr  # pip install rerun-sdk

import numpy.typing as npt
import numpy as np

import itertools
import requests
import argparse
import logging
import tqdm
import cv2
import os

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "hand_gestures"

SAMPLE_IMAGE_PATH = EXAMPLE_DIR / "dataset" / "hand_gestures" / "victory.jpg"

# More samples: 'thumbs_down.jpg', 'victory.jpg', 'pointing_up.jpg', 'thumbs_up.jpg'
SAMPLE_IMAGE_URL = "https://storage.googleapis.com/mediapipe-tasks/gesture_recognizer/victory.jpg"

SAMPLE_VIDEO_PATH = EXAMPLE_DIR / "dataset" / "hand_gestures" / "peace.mp4"

SAMPLE_VIDEO_URL = "https://storage.googleapis.com/rerun-example-datasets/hand_gestures/peace.mp4"

# Emojis from https://github.com/googlefonts/noto-emoji/tree/main
GESTURE_URL = ("https://raw.githubusercontent.com/googlefonts/noto-emoji/9cde38ef5ee6f090ce23f9035e494cb390a2b051/png"
               "/128/")
# Mapping of gesture categories to corresponding emojis
GESTURE_PICTURES = {
    "None": "emoji_u2754.png",
    "Closed_Fist": "emoji_u270a.png",
    "Open_Palm": "emoji_u270b.png",
    "Pointing_Up": "emoji_u261d.png",
    "Thumb_Down": "emoji_u1f44e.png",
    "Thumb_Up": "emoji_u1f44d.png",
    "Victory": "emoji_u270c.png",
    "ILoveYou": "emoji_u1f91f.png"
}


class GestureDetectorLogger:
    """
        Logger for the MediaPipe Gesture Detection solution.
        This class provides logging and utility functions for handling gesture recognition.

        For more information on MediaPipe Gesture Detection:
        https://developers.google.com/mediapipe/solutions/vision/gesture_recognizer
    """

    # URL to the pre-trained MediaPipe Gesture Detection model
    MODEL_DIR: Final = EXAMPLE_DIR / "model"
    MODEL_PATH: Final = (MODEL_DIR / "gesture_recognizer.task").resolve()
    MODEL_URL: Final = (
        "https://storage.googleapis.com/mediapipe-models/gesture_recognizer/gesture_recognizer/float16/latest/gesture_recognizer.task"
    )

    def __init__(self, video_mode: bool = False):
        self._video_mode = video_mode

        if not self.MODEL_PATH.exists():
            download_file(self.MODEL_URL, self.MODEL_PATH)

        base_options = python.BaseOptions(
            model_asset_path=str(self.MODEL_PATH)
        )
        options = vision.GestureRecognizerOptions(
            base_options=base_options,
            running_mode=mp.tasks.vision.RunningMode.VIDEO if self._video_mode else mp.tasks.vision.RunningMode.IMAGE
        )
        self.recognizer = vision.GestureRecognizer.create_from_options(options)

        rr.log(
            "/",
            rr.AnnotationContext(
                rr.ClassDescription(
                    info=rr.AnnotationInfo(id=0, label="Hand3D"),
                    keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS
                )
            ),
            timeless=True,
        )
        # rr.log("Hand3D", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, timeless=True)
        rr.log("Hand3D", rr.ViewCoordinates.LEFT_HAND_Y_DOWN, timeless=True)

    @staticmethod
    def convert_landmarks_to_image_coordinates(hand_landmarks, width, height):
        return [(int(lm.x * width), int(lm.y * height)) for hand_landmark in hand_landmarks for lm in hand_landmark]

    @staticmethod
    def convert_landmarks_to_3d(hand_landmarks):
        return [(lm.x, lm.y, lm.y) for hand_landmark in hand_landmarks for lm in hand_landmark]

    def detect_and_log(self, image: npt.NDArray[np.uint8], frame_time_nano: int | None) -> None:
        # Recognize gestures in the image
        height, width, _ = image.shape
        image = mp.Image(image_format=mp.ImageFormat.SRGB, data=image)

        recognition_result = (
            self.recognizer.recognize_for_video(image, int(frame_time_nano / 1e6))
            if self._video_mode
            else self.recognizer.recognize(image)
        )

        for log_key in ["Media/Points", "Hand/Points", "Media/Connections", "Hand/Connections", "Hand3D/Points"]:
            rr.log(log_key, rr.Clear(recursive=True))

        for i, gesture in enumerate(recognition_result.gestures):
            # Get the top gesture from the recognition result
            gesture_category = gesture[0].category_name if recognition_result.gestures else "None"
            self.present_detected_gesture(gesture_category)  # Log the detected gesture

        if recognition_result.hand_landmarks:
            hand_landmarks = recognition_result.hand_landmarks

            landmark_positions_3d = self.convert_landmarks_to_3d(hand_landmarks)
            if landmark_positions_3d is not None:
                rr.log(
                    "Hand3D/Points",
                    rr.Points3D(landmark_positions_3d, radii=20, class_ids=0,
                                keypoint_ids=[i for i in range(len(landmark_positions_3d))]),
                )

            # Convert normalized coordinates to image coordinates
            points = self.convert_landmarks_to_image_coordinates(hand_landmarks, width, height)

            # Log points to the image and Hand Entity
            for log_key in ["Media/Points", "Hand/Points"]:
                rr.log(
                    log_key,
                    rr.Points2D(points, radii=10, colors=[255, 0, 0])
                )

            # Obtain hand connections from MediaPipe
            mp_hands_connections = mp.solutions.hands.HAND_CONNECTIONS
            points1 = [points[connection[0]] for connection in mp_hands_connections]
            points2 = [points[connection[1]] for connection in mp_hands_connections]

            # Log connections to the image and Hand Entity [128, 128, 128]
            for log_key in ["Media/Connections", "Hand/Connections"]:
                rr.log(
                    log_key,
                    rr.LineStrips2D(
                        np.stack((points1, points2), axis=1),
                        colors=[255, 165, 0]
                    )
                )

    def present_detected_gesture(self, category):
        # Get the corresponding ulr of the picture for the detected gesture category
        gesture_pic = GESTURE_PICTURES.get(
            category,
            "emoji_u2754.png"  # default
        )

        # Log the detection by using the appropriate image
        rr.log(
            "Detection",
            rr.TextDocument(
                f'![Image]({GESTURE_URL + gesture_pic})'.strip(),
                media_type=rr.MediaType.MARKDOWN
            )
        )


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


def resize_image(image: npt.NDArray[np.uint8], max_dim: int | None) -> npt.NDArray[np.uint8]:
    """Resize an image if it is larger than max_dim."""
    if max_dim is None:
        return image
    height, width, _ = image.shape
    scale = max_dim / max(height, width)
    if scale < 1:
        image = cv2.resize(image, (0, 0), fx=scale, fy=scale)
    return image


def run_from_sample_image(path) -> None:
    """Run the gesture recognition on a single image."""
    image = cv2.imread(str(path))
    # image = resize_image(image, max_dim)
    show_image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    rr.log(
        "Media/Image",
        rr.Image(show_image)
    )
    logger = GestureDetectorLogger(video_mode=False)
    logger.detect_and_log(show_image, 0)


def run_from_video_capture(vid: int | str, max_frame_count: int | None) -> None:
    """
    Run the detector on a video stream.

    Parameters
    ----------
    vid:
        The video stream to run the detector on. Use 0/1 for the default camera or a path to a video file.
    max_frame_count:
        The maximum number of frames to process. If None, process all frames.
    """
    cap = cv2.VideoCapture(vid)
    fps = cap.get(cv2.CAP_PROP_FPS)

    detector = GestureDetectorLogger(video_mode=True)

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

            # frame = resize_image(frame, max_dim)

            # get frame time
            frame_time_nano = int(cap.get(cv2.CAP_PROP_POS_MSEC) * 1e6)
            if frame_time_nano == 0:
                # On some platforms it always returns zero, so we compute from the frame counter and fps
                frame_time_nano = int(frame_idx * 1000 / fps * 1e6)

            # convert to rgb
            frame = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)

            # log data
            rr.set_time_sequence("frame_nr", frame_idx)
            rr.set_time_nanos("frame_time", frame_time_nano)
            detector.detect_and_log(frame, frame_time_nano)
            rr.log(
                "Media/Video",
                rr.Image(frame)
            )

    except KeyboardInterrupt:
        pass

    # When everything done, release the capture
    cap.release()
    cv2.destroyAllWindows()


def main() -> None:
    # Ensure the logging gets written to stderr
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    # Set up argument parser with description
    parser = argparse.ArgumentParser(
        description="Uses the MediaPipe Gesture Recognition to track a hand and recognise gestures in image or video."
    )

    parser.add_argument(
        "--demo-image",
        action="store_true",
        help="Run on a demo image automatically downloaded",
    )
    parser.add_argument(
        "--demo-video",
        action="store_true",
        help="Run on a demo image automatically downloaded."
    )
    parser.add_argument(
        "--image",
        type=Path,
        help="Run on the provided image",
    )
    parser.add_argument(
        "--video",
        type=Path,
        help="Run on the provided video file."
    )
    parser.add_argument(
        "--camera",
        type=int,
        default=0,
        help="Run from the camera stream (parameter is the camera ID, usually 0; or maybe 1 on mac)"
    )
    parser.add_argument(
        "--max-frame",
        type=int,
        help="Stop after processing this many frames. If not specified, will run until interrupted.",
    )

    # Add Rerun specific arguments
    rr.script_add_args(parser)

    # Parse command line arguments
    args, unknown = parser.parse_known_args()
    for arg in unknown:  # Log any unknown arguments
        logging.warning(f"unknown arg: {arg}")

    # Set up Rerun with script name
    rr.script_setup(args, "rerun_example_mp_gesture_recognition")

    # Choose the appropriate run mode based on provided arguments
    if args.demo_image:
        if not SAMPLE_IMAGE_PATH.exists():
            download_file(SAMPLE_IMAGE_URL, SAMPLE_IMAGE_PATH)
        run_from_sample_image(SAMPLE_IMAGE_PATH)
    elif args.demo_video:
        if not SAMPLE_VIDEO_PATH.exists():
            download_file(SAMPLE_VIDEO_URL, SAMPLE_VIDEO_PATH)
        run_from_video_capture(str(SAMPLE_VIDEO_PATH), args.max_frame)
    elif args.image:
        run_from_sample_image(args.image)
    elif args.video:
        run_from_video_capture(args.video, args.max_frame)
    elif args.camera:
        run_from_video_capture(int(args.camera), args.max_frame)
    else:
        if not SAMPLE_VIDEO_PATH.exists():
            download_file(SAMPLE_VIDEO_URL, SAMPLE_VIDEO_PATH)
        run_from_video_capture(str(SAMPLE_VIDEO_PATH), args.max_frame)

    # Tear down Rerun script
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
