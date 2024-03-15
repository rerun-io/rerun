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

Use the [MediaPipe](https://google.github.io/mediapipe/) Gesture and landmark detection solutions to
track hands and recognize gestures in images, video, and camera stream.

# Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`LineStrips2D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips2d), [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

# Logging and Visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

## Timelines

For each processed video frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time_sequence("frame_nr", frame_idx)
rr.set_time_nanos("frame_time", frame_time_nano)
```

## Video
The input video is logged as a sequence of [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the `Media/Video` entity.
```python
rr.log(
    "Media/Video",
    rr.Image(frame).compress(jpeg_quality=75)
)
```

## Hand Landmark Points
Logging the hand landmarks involves specifying connections between the points, extracting pose landmark points and logging them to the Rerun SDK.
The 2D points are visualized over the video and at a separate entity. 
Meanwhile, the 3D points allows the creation of a 3D model of the hand for a more comprehensive representation of the hand landmarks.

The 2D and 3D points are logged through a combination of two archetypes.
For the 2D points, the Points2D and LineStrips2D archetypes are utilized. These archetypes help visualize the points and connect them with lines, respectively.
As for the 3D points, the logging process involves two steps. First, a timeless [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged, that contains the information which maps keypoint ids to labels and how to connect
the keypoints. Defining these connections automatically renders lines between them. Mediapipe provides the `HAND_CONNECTIONS` variable which contains the list of `(from, to)` landmark indices that define the connections. 
Second, the actual keypoint positions are logged in 3D [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype.

### Label Mapping and Keypoint Connections

```python
rr.log(
    "/",
    rr.AnnotationContext(
        rr.ClassDescription(
            info=rr.AnnotationInfo(id=0, label="Hand3D"),
            keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS,
        )
    ),
    timeless=True,
)

rr.log("Hand3D", rr.ViewCoordinates.LEFT_HAND_Y_DOWN, timeless=True)
```


### 2D Points

```python
# Log points to the image and Hand Entity
for log_key in ["Media/Points", "Hand/Points"]:
    rr.log(
      log_key, 
      rr.Points2D(points, radii=10, colors=[255, 0, 0])
    )

# Log connections to the image and Hand Entity [128, 128, 128]
for log_key in ["Media/Connections", "Hand/Connections"]:
    rr.log(
      log_key, 
      rr.LineStrips2D(np.stack((points1, points2), axis=1), colors=[255, 165, 0])
    )
```

### 3D Points

```python
rr.log(
    "Hand3D/Points",
    rr.Points3D(
        landmark_positions_3d,
        radii=20,
        class_ids=0,
        keypoint_ids=[i for i in range(len(landmark_positions_3d))],
    ),
)
```

## Detection 

To showcase gesture recognition, an image of the corresponding gesture emoji is displayed within a `TextDocument` under the `Detection` entity.

```python
# Log the detection by using the appropriate image
rr.log(
    "Detection",
    rr.TextDocument(f"![Image]({GESTURE_URL + gesture_pic})".strip(), media_type=rr.MediaType.MARKDOWN),
)
```

# Run the Code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/gesture_detection/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/gesture_detection/main.py # run the example
```
If you wish to customize it for various videos, adjust the maximum frames, explore additional features, or save it use the CLI with the `--help` option for guidance:
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

[//]: # ()
[//]: # ()
[//]: # ()
[//]: # ()
[//]: # (# Run)

[//]: # ()
[//]: # ()
[//]: # (```bash)

[//]: # ()
[//]: # (# Install the required Python packages specified in the requirements file)

[//]: # ()
[//]: # (pip install -r examples/python/gesture_detection/requirements.txt)

[//]: # ()
[//]: # (python examples/python/gesture_detection/main.py)

[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (# Usage)

[//]: # ()
[//]: # ()
[//]: # (CLI usage help is available using the `--help` option:)

[//]: # ()
[//]: # ()
[//]: # (```bash)

[//]: # ()
[//]: # ($ python examples/python/gesture_detection/main.py --help)

[//]: # ()
[//]: # (usage: main.py [-h] [--demo-image] [--demo-video] [--image IMAGE])

[//]: # ()
[//]: # (               [--video VIDEO] [--camera CAMERA] [--max-frame MAX_FRAME])

[//]: # ()
[//]: # (               [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE])

[//]: # ()
[//]: # (               [-o])

[//]: # ()
[//]: # ()
[//]: # (Uses the MediaPipe Gesture Recognition to track a hand and recognize gestures)

[//]: # ()
[//]: # (in image or video.)

[//]: # ()
[//]: # ()
[//]: # (optional arguments:)

[//]: # ()
[//]: # (  -h, --help            show this help message and exit)

[//]: # ()
[//]: # (  --demo-image          Run on a demo image automatically downloaded)

[//]: # ()
[//]: # (  --demo-video          Run on a demo image automatically downloaded.)

[//]: # ()
[//]: # (  --image IMAGE         Run on the provided image)

[//]: # ()
[//]: # (  --video VIDEO         Run on the provided video file.)

[//]: # ()
[//]: # (  --camera CAMERA       Run from the camera stream &#40;parameter is the camera)

[//]: # ()
[//]: # (                        ID, usually 0; or maybe 1 on mac&#41;)

[//]: # ()
[//]: # (  --max-frame MAX_FRAME)

[//]: # ()
[//]: # (                        Stop after processing this many frames. If not)

[//]: # ()
[//]: # (                        specified, will run until interrupted.)

[//]: # ()
[//]: # (  --headless            Don\'t show GUI)

[//]: # ()
[//]: # (  --connect             Connect to an external viewer)

[//]: # ()
[//]: # (  --serve               Serve a web viewer &#40;WARNING: experimental feature&#41;)

[//]: # ()
[//]: # (  --addr ADDR           Connect to this ip:port)

[//]: # ()
[//]: # (  --save SAVE           Save data to a .rrd file at this path)

[//]: # ()
[//]: # (  -o, --stdout          Log data to standard output, to be piped into a Rerun)

[//]: # ()
[//]: # (                        Viewer)

[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (Here is an overview of the options specific to this example:)

[//]: # ()
[//]: # ()
[//]: # (- ***Running modes*:** By default, this example streams images from the default webcam. Another webcam can be used by)

[//]: # ()
[//]: # (  providing a camera index with the `--camera` option. Alternatively, images can be read from a video file &#40;)

[//]: # ()
[//]: # (  using `--video PATH`&#41; or a single image file &#40;using `-image PATH`&#41;. Also, a demo image can be automatically downloaded)

[//]: # ()
[//]: # (  and used with `--demo-image`. Also, a demo video can be automatically downloaded and used with `--demo-video`.)

[//]: # ()
[//]: # (- ***Limiting frame count*:** When running from a webcam or a video file, this example can be set to stop after a given)

[//]: # ()
[//]: # (  number of frames using `--max-frame MAX_FRAME`.)

[//]: # ()
[//]: # ()
[//]: # (# Overview)

[//]: # ()
[//]: # ()
[//]: # (Logging Details:)

[//]: # ()
[//]: # ()
[//]: # (1. Hand Landmarks as 2D Points:)

[//]: # ()
[//]: # ()
[//]: # (    - Extracts hand landmark points as normalized 2D coordinates.)

[//]: # ()
[//]: # ()
[//]: # (    - Utilizes image width and height for conversion into image coordinates.)

[//]: # ()
[//]: # ()
[//]: # (    - Logs the 2D points to the Rerun SDK.)

[//]: # ()
[//]: # ()
[//]: # ()
[//]: # (2. Hand Landmarks as 3D Points:)

[//]: # ()
[//]: # ()
[//]: # (    - Detects hand landmarks using MediaPipe solutions.)

[//]: # ()
[//]: # ()
[//]: # (    - Converts the detected hand landmarks into 3D coordinates.)

[//]: # ()
[//]: # ()
[//]: # (    - Logs the 3D points to the Rerun SDK.)

[//]: # ()
[//]: # ()
[//]: # ()
[//]: # (3. Gesture Detection Results:)

[//]: # ()
[//]: # ()
[//]: # (    - Utilizes the Gesture Detection solution from MediaPipe.)

[//]: # ()
[//]: # ()
[//]: # (    - Logs the results of gesture detection as emoji)

[//]: # ()
[//]: # ()
[//]: # (# Logging Data)

[//]: # ()
[//]: # ()
[//]: # (## Timelines for Video)

[//]: # ()
[//]: # ()
[//]: # (You can utilize Rerun timelines' functions to associate data with one or more timelines. As a result, each frame of the)

[//]: # ()
[//]: # (video can be linked with its corresponding timestamp.)

[//]: # ()
[//]: # ()
[//]: # (```python)

[//]: # ()
[//]: # (def run_from_video_capture&#40;vid: int | str, max_frame_count: int | None&#41; -> None:)

[//]: # ()
[//]: # (    """)

[//]: # ()
[//]: # (    Run the detector on a video stream.)

[//]: # ()
[//]: # ()
[//]: # (    Parameters)

[//]: # ()
[//]: # (    ----------)

[//]: # ()
[//]: # (    vid:)

[//]: # ()
[//]: # (        The video stream to run the detector on. Use 0/1 for the default camera or a path to a video file.)

[//]: # ()
[//]: # (    max_frame_count:)

[//]: # ()
[//]: # (        The maximum number of frames to process. If None, process all frames.)

[//]: # ()
[//]: # (    """)

[//]: # ()
[//]: # (    cap = cv2.VideoCapture&#40;vid&#41;)

[//]: # ()
[//]: # (    fps = cap.get&#40;cv2.CAP_PROP_FPS&#41;)

[//]: # ()
[//]: # ()
[//]: # (    detector = GestureDetectorLogger&#40;video_mode=True&#41;)

[//]: # ()
[//]: # ()
[//]: # (    try:)

[//]: # ()
[//]: # (        it: Iterable[int] = itertools.count&#40;&#41; if max_frame_count is None else range&#40;max_frame_count&#41;)

[//]: # ()
[//]: # ()
[//]: # (        for frame_idx in tqdm.tqdm&#40;it, desc="Processing frames"&#41;:)

[//]: # ()
[//]: # (            ret, frame = cap.read&#40;&#41;)

[//]: # ()
[//]: # (            if not ret:)

[//]: # ()
[//]: # (                break)

[//]: # ()
[//]: # ()
[//]: # (            if np.all&#40;frame == 0&#41;:)

[//]: # ()
[//]: # (                continue)

[//]: # ()
[//]: # ()
[//]: # (            frame_time_nano = int&#40;cap.get&#40;cv2.CAP_PROP_POS_MSEC&#41; * 1e6&#41;)

[//]: # ()
[//]: # (            if frame_time_nano == 0:)

[//]: # ()
[//]: # (                frame_time_nano = int&#40;frame_idx * 1000 / fps * 1e6&#41;)

[//]: # ()
[//]: # ()
[//]: # (            frame = cv2.cvtColor&#40;frame, cv2.COLOR_BGR2RGB&#41;)

[//]: # ()
[//]: # ()
[//]: # (            rr.set_time_sequence&#40;"frame_nr", frame_idx&#41;)

[//]: # ()
[//]: # (            rr.set_time_nanos&#40;"frame_time", frame_time_nano&#41;)

[//]: # ()
[//]: # (            detector.detect_and_log&#40;frame, frame_time_nano&#41;)

[//]: # ()
[//]: # (            rr.log&#40;)

[//]: # ()
[//]: # (                "Media/Video",)

[//]: # ()
[//]: # (                rr.Image&#40;frame&#41;)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # ()
[//]: # (    except KeyboardInterrupt:)

[//]: # ()
[//]: # (        pass)

[//]: # ()
[//]: # ()
[//]: # (    cap.release&#40;&#41;)

[//]: # ()
[//]: # (    cv2.destroyAllWindows&#40;&#41;)

[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (## Hand Landmarks as 2D Points)

[//]: # ()
[//]: # ()
[//]: # (![gesture_recognition_2d_points]&#40;https://github.com/rerun-io/rerun/assets/49308613/7e5dd809-be06-4f62-93a8-4fc03e5dfa0e&#41;)

[//]: # ()
[//]: # ()
[//]: # (You can extract hand landmark points as normalized values, utilizing the image's width and height for conversion into)

[//]: # ()
[//]: # (image coordinates. These coordinates are then logged as 2D points to the Rerun SDK. Additionally, you can identify)

[//]: # ()
[//]: # (connections between the landmarks and log them as 2D linestrips.)

[//]: # ()
[//]: # ()
[//]: # (```python)

[//]: # ()
[//]: # (class GestureDetectorLogger:)

[//]: # ()
[//]: # ()
[//]: # (    def detect_and_log&#40;self, image: npt.NDArray[np.uint8], frame_time_nano: int | None&#41; -> None:)

[//]: # ()
[//]: # (        # Recognize gestures in the image)

[//]: # ()
[//]: # (        height, width, _ = image.shape)

[//]: # ()
[//]: # (        image = mp.Image&#40;image_format=mp.ImageFormat.SRGB, data=image&#41;)

[//]: # ()
[//]: # ()
[//]: # (        recognition_result = &#40;)

[//]: # ()
[//]: # (            self.recognizer.recognize_for_video&#40;image, int&#40;frame_time_nano / 1e6&#41;&#41;)

[//]: # ()
[//]: # (            if self._video_mode)

[//]: # ()
[//]: # (            else self.recognizer.recognize&#40;image&#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (        # Clear the values)

[//]: # ()
[//]: # (        for log_key in ["Media/Points", "Media/Connections"]:)

[//]: # ()
[//]: # (            rr.log&#40;log_key, rr.Clear&#40;recursive=True&#41;&#41;)

[//]: # ()
[//]: # ()
[//]: # (        if recognition_result.hand_landmarks:)

[//]: # ()
[//]: # (            hand_landmarks = recognition_result.hand_landmarks)

[//]: # ()
[//]: # ()
[//]: # (            # Convert normalized coordinates to image coordinates)

[//]: # ()
[//]: # (            points = self.convert_landmarks_to_image_coordinates&#40;hand_landmarks, width, height&#41;)

[//]: # ()
[//]: # ()
[//]: # (            # Log points to the image and Hand Entity)

[//]: # ()
[//]: # (            rr.log&#40;)

[//]: # ()
[//]: # (                "Media/Points",)

[//]: # ()
[//]: # (                rr.Points2D&#40;points, radii=10, colors=[255, 0, 0]&#41;)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # ()
[//]: # (            # Obtain hand connections from MediaPipe)

[//]: # ()
[//]: # (            mp_hands_connections = mp.solutions.hands.HAND_CONNECTIONS)

[//]: # ()
[//]: # (            points1 = [points[connection[0]] for connection in mp_hands_connections])

[//]: # ()
[//]: # (            points2 = [points[connection[1]] for connection in mp_hands_connections])

[//]: # ()
[//]: # ()
[//]: # (            # Log connections to the image and Hand Entity)

[//]: # ()
[//]: # (            rr.log&#40;)

[//]: # ()
[//]: # (                "Media/Connections",)

[//]: # ()
[//]: # (                rr.LineStrips2D&#40;)

[//]: # ()
[//]: # (                    np.stack&#40;&#40;points1, points2&#41;, axis=1&#41;,)

[//]: # ()
[//]: # (                    colors=[255, 165, 0])

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (## Hand Landmarks as 3D Points)

[//]: # ()
[//]: # ()
[//]: # (![gesture_recognition_3d_points]&#40;https://github.com/rerun-io/rerun/assets/49308613/b24bb0e5-57cc-43f0-948b-3480fe9073a2&#41;)

[//]: # ()
[//]: # ()
[//]: # (You can first define the connections between the points using keypoints from Annotation Context in the init function,)

[//]: # ()
[//]: # (and then log them as 3D points.)

[//]: # ()
[//]: # ()
[//]: # (```python)

[//]: # ()
[//]: # ()
[//]: # (class GestureDetectorLogger:)

[//]: # ()
[//]: # ()
[//]: # (    def __init__&#40;self, video_mode: bool = False&#41;:)

[//]: # ()
[//]: # (        # … existing code …)

[//]: # ()
[//]: # (        rr.log&#40;)

[//]: # ()
[//]: # (            "/",)

[//]: # ()
[//]: # (            rr.AnnotationContext&#40;)

[//]: # ()
[//]: # (                rr.ClassDescription&#40;)

[//]: # ()
[//]: # (                    info=rr.AnnotationInfo&#40;id=0, label="Hand3D"&#41;,)

[//]: # ()
[//]: # (                    keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS)

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # (            &#41;,)

[//]: # ()
[//]: # (            timeless=True,)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # (        rr.log&#40;"Hand3D", rr.ViewCoordinates.RIGHT_HAND_X_DOWN, timeless=True&#41;)

[//]: # ()
[//]: # ()
[//]: # ()
[//]: # (def detect_and_log&#40;self, image: npt.NDArray[np.uint8], frame_time_nano: int | None&#41; -> None:)

[//]: # ()
[//]: # (    # … existing code …)

[//]: # ()
[//]: # ()
[//]: # (    if recognition_result.hand_landmarks:)

[//]: # ()
[//]: # (        hand_landmarks = recognition_result.hand_landmarks)

[//]: # ()
[//]: # ()
[//]: # (        landmark_positions_3d = self.convert_landmarks_to_3d&#40;hand_landmarks&#41;)

[//]: # ()
[//]: # (        if landmark_positions_3d is not None:)

[//]: # ()
[//]: # (            rr.log&#40;)

[//]: # ()
[//]: # (                "Hand3D/Points",)

[//]: # ()
[//]: # (                rr.Points3D&#40;landmark_positions_3d, radii=20, class_ids=0,)

[//]: # ()
[//]: # (                            keypoint_ids=[i for i in range&#40;len&#40;landmark_positions_3d&#41;&#41;]&#41;,)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # ()
[//]: # (    # … existing code …)

[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (## Gesture Detection Presentation)

[//]: # ()
[//]: # ()
[//]: # (![Gesture Detection Presentation]&#40;https://github.com/rerun-io/rerun/assets/49308613/32cc44f4-28e5-4ed1-b283-f7351a087535&#41;)

[//]: # ()
[//]: # ()
[//]: # (One effective method to present these results to the viewer is by utilizing a TextDocument along with emojis for)

[//]: # ()
[//]: # (enhanced visual communication.)

[//]: # ()
[//]: # ()
[//]: # (```python)

[//]: # ()
[//]: # ()
[//]: # (# Emojis from https://github.com/googlefonts/noto-emoji/tree/main)

[//]: # ()
[//]: # (GESTURE_URL = "https://raw.githubusercontent.com/googlefonts/noto-emoji/9cde38ef5ee6f090ce23f9035e494cb390a2b051/png/128/")

[//]: # ()
[//]: # ()
[//]: # (# Mapping of gesture categories to corresponding emojis)

[//]: # ()
[//]: # (GESTURE_PICTURES = {)

[//]: # ()
[//]: # (    "None": "emoji_u2754.png",)

[//]: # ()
[//]: # (    "Closed_Fist": "emoji_u270a.png",)

[//]: # ()
[//]: # (    "Open_Palm": "emoji_u270b.png",)

[//]: # ()
[//]: # (    "Pointing_Up": "emoji_u261d.png",)

[//]: # ()
[//]: # (    "Thumb_Down": "emoji_u1f44e.png",)

[//]: # ()
[//]: # (    "Thumb_Up": "emoji_u1f44d.png",)

[//]: # ()
[//]: # (    "Victory": "emoji_u270c.png",)

[//]: # ()
[//]: # (    "ILoveYou": "emoji_u1f91f.png")

[//]: # ()
[//]: # (})

[//]: # ()
[//]: # ()
[//]: # ()
[//]: # (class GestureDetectorLogger:)

[//]: # ()
[//]: # ()
[//]: # (    def detect_and_log&#40;self, image: npt.NDArray[np.uint8], frame_time_nano: int | None&#41; -> None:)

[//]: # ()
[//]: # (        # Recognize gestures in the image)

[//]: # ()
[//]: # (        height, width, _ = image.shape)

[//]: # ()
[//]: # (        image = mp.Image&#40;image_format=mp.ImageFormat.SRGB, data=image&#41;)

[//]: # ()
[//]: # ()
[//]: # (        recognition_result = &#40;)

[//]: # ()
[//]: # (            self.recognizer.recognize_for_video&#40;image, int&#40;frame_time_nano / 1e6&#41;&#41;)

[//]: # ()
[//]: # (            if self._video_mode)

[//]: # ()
[//]: # (            else self.recognizer.recognize&#40;image&#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (        for log_key in ["Media/Points", "Hand/Points", "Media/Connections", "Hand/Connections", "Hand3D/Points"]:)

[//]: # ()
[//]: # (            rr.log&#40;log_key, rr.Clear&#40;recursive=True&#41;&#41;)

[//]: # ()
[//]: # ()
[//]: # (        for i, gesture in enumerate&#40;recognition_result.gestures&#41;:)

[//]: # ()
[//]: # (            # Get the top gesture from the recognition result)

[//]: # ()
[//]: # (            gesture_category = gesture[0].category_name if recognition_result.gestures else "None")

[//]: # ()
[//]: # (            self.present_detected_gesture&#40;gesture_category&#41;  # Log the detected gesture)

[//]: # ()
[//]: # ()
[//]: # (    def present_detected_gesture&#40;self, category&#41;:)

[//]: # ()
[//]: # (        # Get the corresponding ulr of the picture for the detected gesture category)

[//]: # ()
[//]: # (        gesture_pic = GESTURE_PICTURES.get&#40;)

[//]: # ()
[//]: # (            category,)

[//]: # ()
[//]: # (            "emoji_u2754.png"  # default)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (        # Log the detection by using the appropriate image)

[//]: # ()
[//]: # (        rr.log&#40;)

[//]: # ()
[//]: # (            "Detection",)

[//]: # ()
[//]: # (            rr.TextDocument&#40;)

[//]: # ()
[//]: # (                f'![Image]&#40;{GESTURE_URL + gesture_pic}&#41;'.strip&#40;&#41;,)

[//]: # ()
[//]: # (                media_type=rr.MediaType.MARKDOWN)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (```)

[//]: # ()
[//]: # ()
[//]: # (# Gesture Detector Logger)

[//]: # ()
[//]: # ()
[//]: # (```python)

[//]: # ()
[//]: # ()
[//]: # (class GestureDetectorLogger:)

[//]: # ()
[//]: # (    """)

[//]: # ()
[//]: # (        Logger for the MediaPipe Gesture Detection solution.)

[//]: # ()
[//]: # (        This class provides logging and utility functions for handling gesture recognition.)

[//]: # ()
[//]: # ()
[//]: # (        For more information on MediaPipe Gesture Detection:)

[//]: # ()
[//]: # (        https://developers.google.com/mediapipe/solutions/vision/gesture_recognizer)

[//]: # ()
[//]: # (    """)

[//]: # ()
[//]: # ()
[//]: # (    # URL to the pre-trained MediaPipe Gesture Detection model)

[//]: # ()
[//]: # (    MODEL_DIR: Final = EXAMPLE_DIR / "model")

[//]: # ()
[//]: # (    MODEL_PATH: Final = &#40;MODEL_DIR / "gesture_recognizer.task"&#41;.resolve&#40;&#41;)

[//]: # ()
[//]: # (    MODEL_URL: Final = &#40;)

[//]: # ()
[//]: # (        "https://storage.googleapis.com/mediapipe-models/gesture_recognizer/gesture_recognizer/float16/latest/gesture_recognizer.task")

[//]: # ()
[//]: # (    &#41;)

[//]: # ()
[//]: # ()
[//]: # (    def __init__&#40;self, video_mode: bool = False&#41;:)

[//]: # ()
[//]: # (        self._video_mode = video_mode)

[//]: # ()
[//]: # ()
[//]: # (        if not self.MODEL_PATH.exists&#40;&#41;:)

[//]: # ()
[//]: # (            download_file&#40;self.MODEL_URL, self.MODEL_PATH&#41;)

[//]: # ()
[//]: # ()
[//]: # (        base_options = python.BaseOptions&#40;)

[//]: # ()
[//]: # (            model_asset_path=str&#40;self.MODEL_PATH&#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # (        options = vision.GestureRecognizerOptions&#40;)

[//]: # ()
[//]: # (            base_options=base_options,)

[//]: # ()
[//]: # (            running_mode=mp.tasks.vision.RunningMode.VIDEO if self._video_mode else mp.tasks.vision.RunningMode.IMAGE)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # (        self.recognizer = vision.GestureRecognizer.create_from_options&#40;options&#41;)

[//]: # ()
[//]: # ()
[//]: # (        rr.log&#40;)

[//]: # ()
[//]: # (            "/",)

[//]: # ()
[//]: # (            rr.AnnotationContext&#40;)

[//]: # ()
[//]: # (                rr.ClassDescription&#40;)

[//]: # ()
[//]: # (                    info=rr.AnnotationInfo&#40;id=0, label="Hand3D"&#41;,)

[//]: # ()
[//]: # (                    keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS)

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # (            &#41;,)

[//]: # ()
[//]: # (            timeless=True,)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # (        # rr.log&#40;"Hand3D", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, timeless=True&#41;)

[//]: # ()
[//]: # (        rr.log&#40;"Hand3D", rr.ViewCoordinates.LEFT_HAND_Y_DOWN, timeless=True&#41;)

[//]: # ()
[//]: # ()
[//]: # (    @staticmethod)

[//]: # ()
[//]: # (    def convert_landmarks_to_image_coordinates&#40;hand_landmarks, width, height&#41;:)

[//]: # ()
[//]: # (        return [&#40;int&#40;lm.x * width&#41;, int&#40;lm.y * height&#41;&#41; for hand_landmark in hand_landmarks for lm in hand_landmark])

[//]: # ()
[//]: # ()
[//]: # (    @staticmethod)

[//]: # ()
[//]: # (    def convert_landmarks_to_3d&#40;hand_landmarks&#41;:)

[//]: # ()
[//]: # (        return [&#40;lm.x, lm.y, lm.y&#41; for hand_landmark in hand_landmarks for lm in hand_landmark])

[//]: # ()
[//]: # ()
[//]: # (    def detect_and_log&#40;self, image: npt.NDArray[np.uint8], frame_time_nano: int | None&#41; -> None:)

[//]: # ()
[//]: # (        # Recognize gestures in the image)

[//]: # ()
[//]: # (        height, width, _ = image.shape)

[//]: # ()
[//]: # (        image = mp.Image&#40;image_format=mp.ImageFormat.SRGB, data=image&#41;)

[//]: # ()
[//]: # ()
[//]: # (        recognition_result = &#40;)

[//]: # ()
[//]: # (            self.recognizer.recognize_for_video&#40;image, int&#40;frame_time_nano / 1e6&#41;&#41;)

[//]: # ()
[//]: # (            if self._video_mode)

[//]: # ()
[//]: # (            else self.recognizer.recognize&#40;image&#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (        for log_key in ["Media/Points", "Hand/Points", "Media/Connections", "Hand/Connections", "Hand3D/Points"]:)

[//]: # ()
[//]: # (            rr.log&#40;log_key, rr.Clear&#40;recursive=True&#41;&#41;)

[//]: # ()
[//]: # ()
[//]: # (        for i, gesture in enumerate&#40;recognition_result.gestures&#41;:)

[//]: # ()
[//]: # (            # Get the top gesture from the recognition result)

[//]: # ()
[//]: # (            gesture_category = gesture[0].category_name if recognition_result.gestures else "None")

[//]: # ()
[//]: # (            self.present_detected_gesture&#40;gesture_category&#41;  # Log the detected gesture)

[//]: # ()
[//]: # ()
[//]: # (        if recognition_result.hand_landmarks:)

[//]: # ()
[//]: # (            hand_landmarks = recognition_result.hand_landmarks)

[//]: # ()
[//]: # ()
[//]: # (            landmark_positions_3d = self.convert_landmarks_to_3d&#40;hand_landmarks&#41;)

[//]: # ()
[//]: # (            if landmark_positions_3d is not None:)

[//]: # ()
[//]: # (                rr.log&#40;)

[//]: # ()
[//]: # (                    "Hand3D/Points",)

[//]: # ()
[//]: # (                    rr.Points3D&#40;landmark_positions_3d, radii=20, class_ids=0,)

[//]: # ()
[//]: # (                                keypoint_ids=[i for i in range&#40;len&#40;landmark_positions_3d&#41;&#41;]&#41;,)

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # ()
[//]: # (            # Convert normalized coordinates to image coordinates)

[//]: # ()
[//]: # (            points = self.convert_landmarks_to_image_coordinates&#40;hand_landmarks, width, height&#41;)

[//]: # ()
[//]: # ()
[//]: # (            # Log points to the image and Hand Entity)

[//]: # ()
[//]: # (            for log_key in ["Media/Points", "Hand/Points"]:)

[//]: # ()
[//]: # (                rr.log&#40;)

[//]: # ()
[//]: # (                    log_key,)

[//]: # ()
[//]: # (                    rr.Points2D&#40;points, radii=10, colors=[255, 0, 0]&#41;)

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # ()
[//]: # (            # Obtain hand connections from MediaPipe)

[//]: # ()
[//]: # (            mp_hands_connections = mp.solutions.hands.HAND_CONNECTIONS)

[//]: # ()
[//]: # (            points1 = [points[connection[0]] for connection in mp_hands_connections])

[//]: # ()
[//]: # (            points2 = [points[connection[1]] for connection in mp_hands_connections])

[//]: # ()
[//]: # ()
[//]: # (            # Log connections to the image and Hand Entity [128, 128, 128])

[//]: # ()
[//]: # (            for log_key in ["Media/Connections", "Hand/Connections"]:)

[//]: # ()
[//]: # (                rr.log&#40;)

[//]: # ()
[//]: # (                    log_key,)

[//]: # ()
[//]: # (                    rr.LineStrips2D&#40;)

[//]: # ()
[//]: # (                        np.stack&#40;&#40;points1, points2&#41;, axis=1&#41;,)

[//]: # ()
[//]: # (                        colors=[255, 165, 0])

[//]: # ()
[//]: # (                    &#41;)

[//]: # ()
[//]: # (                &#41;)

[//]: # ()
[//]: # ()
[//]: # (    def present_detected_gesture&#40;self, category&#41;:)

[//]: # ()
[//]: # (        # Get the corresponding ulr of the picture for the detected gesture category)

[//]: # ()
[//]: # (        gesture_pic = GESTURE_PICTURES.get&#40;)

[//]: # ()
[//]: # (            category,)

[//]: # ()
[//]: # (            "emoji_u2754.png"  # default)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (        # Log the detection by using the appropriate image)

[//]: # ()
[//]: # (        rr.log&#40;)

[//]: # ()
[//]: # (            "Detection",)

[//]: # ()
[//]: # (            rr.TextDocument&#40;)

[//]: # ()
[//]: # (                f'![Image]&#40;{GESTURE_URL + gesture_pic}&#41;'.strip&#40;&#41;,)

[//]: # ()
[//]: # (                media_type=rr.MediaType.MARKDOWN)

[//]: # ()
[//]: # (            &#41;)

[//]: # ()
[//]: # (        &#41;)

[//]: # ()
[//]: # ()
[//]: # (```)
