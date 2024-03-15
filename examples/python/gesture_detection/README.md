<!--[metadata]
title = "Hand Tracking and Gesture Recognition"
tags = ["mediapipe", "keypoint-detection", "2D", "3D"]
description = "Use the MediaPipe Gesture Detection solution to track hand and recognize gestures in image/video."
thumbnail = "https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/480w.png"
thumbnail_dimensions = [480, 259]
-->


<picture>
  <img src="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/1200w.png">
</picture>

# Run

```bash
# Install the required Python packages specified in the requirements file
pip install -r examples/python/gesture_detection/requirements.txt
python examples/python/gesture_detection/main.py
```

# Usage

CLI usage help is available using the `--help` option:

```bash
$ python examples/python/gesture_detection/main.py --help
usage: main.py [-h] [--demo-image] [--demo-video] [--image IMAGE]
               [--video VIDEO] [--camera CAMERA] [--max-frame MAX_FRAME]
               [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE]
               [-o]

Uses the MediaPipe Gesture Recognition to track a hand and recognize gestures
in image or video.

optional arguments:
  -h, --help            show this help message and exit
  --demo-image          Run on a demo image automatically downloaded
  --demo-video          Run on a demo image automatically downloaded.
  --image IMAGE         Run on the provided image
  --video VIDEO         Run on the provided video file.
  --camera CAMERA       Run from the camera stream (parameter is the camera
                        ID, usually 0; or maybe 1 on mac)
  --max-frame MAX_FRAME
                        Stop after processing this many frames. If not
                        specified, will run until interrupted.
  --headless            Don\'t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun
                        Viewer
```

Here is an overview of the options specific to this example:

- ***Running modes*:** By default, this example streams images from the default webcam. Another webcam can be used by
  providing a camera index with the `--camera` option. Alternatively, images can be read from a video file (
  using `--video PATH`) or a single image file (using `-image PATH`). Also, a demo image can be automatically downloaded
  and used with `--demo-image`. Also, a demo video can be automatically downloaded and used with `--demo-video`.
- ***Limiting frame count*:** When running from a webcam or a video file, this example can be set to stop after a given
  number of frames using `--max-frame MAX_FRAME`.

# Overview

Use the [MediaPipe](https://google.github.io/mediapipe/)  Gesture detection and Gesture landmark detection solutions to
track hands and recognize gestures in images and videos.

Logging Details:

1. Hand Landmarks as 2D Points:

    - Extracts hand landmark points as normalized 2D coordinates.

    - Utilizes image width and height for conversion into image coordinates.

    - Logs the 2D points to the Rerun SDK.


2. Hand Landmarks as 3D Points:

    - Detects hand landmarks using MediaPipe solutions.

    - Converts the detected hand landmarks into 3D coordinates.

    - Logs the 3D points to the Rerun SDK.


3. Gesture Detection Results:

    - Utilizes the Gesture Detection solution from MediaPipe.

    - Logs the results of gesture detection as emoji

# Logging Data

## Timelines for Video

You can utilize Rerun timelines' functions to associate data with one or more timelines. As a result, each frame of the
video can be linked with its corresponding timestamp.

```python
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
            ret, frame = cap.read()
            if not ret:
                break

            if np.all(frame == 0):
                continue

            frame_time_nano = int(cap.get(cv2.CAP_PROP_POS_MSEC) * 1e6)
            if frame_time_nano == 0:
                frame_time_nano = int(frame_idx * 1000 / fps * 1e6)

            frame = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)

            rr.set_time_sequence("frame_nr", frame_idx)
            rr.set_time_nanos("frame_time", frame_time_nano)
            detector.detect_and_log(frame, frame_time_nano)
            rr.log(
                "Media/Video",
                rr.Image(frame)
            )

    except KeyboardInterrupt:
        pass

    cap.release()
    cv2.destroyAllWindows()
```

## Hand Landmarks as 2D Points

![gesture_recognition_2d_points](https://github.com/rerun-io/rerun/assets/49308613/7e5dd809-be06-4f62-93a8-4fc03e5dfa0e)

You can extract hand landmark points as normalized values, utilizing the image's width and height for conversion into
image coordinates. These coordinates are then logged as 2D points to the Rerun SDK. Additionally, you can identify
connections between the landmarks and log them as 2D linestrips.

```python
class GestureDetectorLogger:

    def detect_and_log(self, image: npt.NDArray[np.uint8], frame_time_nano: int | None) -> None:
        # Recognize gestures in the image
        height, width, _ = image.shape
        image = mp.Image(image_format=mp.ImageFormat.SRGB, data=image)

        recognition_result = (
            self.recognizer.recognize_for_video(image, int(frame_time_nano / 1e6))
            if self._video_mode
            else self.recognizer.recognize(image)
        )

        # Clear the values
        for log_key in ["Media/Points", "Media/Connections"]:
            rr.log(log_key, rr.Clear(recursive=True))

        if recognition_result.hand_landmarks:
            hand_landmarks = recognition_result.hand_landmarks

            # Convert normalized coordinates to image coordinates
            points = self.convert_landmarks_to_image_coordinates(hand_landmarks, width, height)

            # Log points to the image and Hand Entity
            rr.log(
                "Media/Points",
                rr.Points2D(points, radii=10, colors=[255, 0, 0])
            )

            # Obtain hand connections from MediaPipe
            mp_hands_connections = mp.solutions.hands.HAND_CONNECTIONS
            points1 = [points[connection[0]] for connection in mp_hands_connections]
            points2 = [points[connection[1]] for connection in mp_hands_connections]

            # Log connections to the image and Hand Entity
            rr.log(
                "Media/Connections",
                rr.LineStrips2D(
                    np.stack((points1, points2), axis=1),
                    colors=[255, 165, 0]
                )
            )
```

## Hand Landmarks as 3D Points

![gesture_recognition_3d_points](https://github.com/rerun-io/rerun/assets/49308613/b24bb0e5-57cc-43f0-948b-3480fe9073a2)

You can first define the connections between the points using keypoints from Annotation Context in the init function,
and then log them as 3D points.

```python

class GestureDetectorLogger:

    def __init__(self, video_mode: bool = False):
        # … existing code …
        rr.log(
            "/",
            rr.AnnotationContext(
                rr.ClassDescription(
                    info=rr.AnnotationInfo(id=0, label="Hand3D"),
                    keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS
                )
            ),
            static=True,
        )
        rr.log("Hand3D", rr.ViewCoordinates.RIGHT_HAND_X_DOWN, static=True)


def detect_and_log(self, image: npt.NDArray[np.uint8], frame_time_nano: int | None) -> None:
    # … existing code …

    if recognition_result.hand_landmarks:
        hand_landmarks = recognition_result.hand_landmarks

        landmark_positions_3d = self.convert_landmarks_to_3d(hand_landmarks)
        if landmark_positions_3d is not None:
            rr.log(
                "Hand3D/Points",
                rr.Points3D(landmark_positions_3d, radii=20, class_ids=0,
                            keypoint_ids=[i for i in range(len(landmark_positions_3d))]),
            )

    # … existing code …
```

## Gesture Detection Presentation

![Gesture Detection Presentation](https://github.com/rerun-io/rerun/assets/49308613/32cc44f4-28e5-4ed1-b283-f7351a087535)

One effective method to present these results to the viewer is by utilizing a TextDocument along with emojis for
enhanced visual communication.

```python

# Emojis from https://github.com/googlefonts/noto-emoji/tree/main
GESTURE_URL = "https://raw.githubusercontent.com/googlefonts/noto-emoji/9cde38ef5ee6f090ce23f9035e494cb390a2b051/png/128/"

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

```

# Gesture Detector Logger

```python

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
            static=True,
        )
        # rr.log("Hand3D", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, static=True)
        rr.log("Hand3D", rr.ViewCoordinates.LEFT_HAND_Y_DOWN, static=True)

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

```
