<!--[metadata]
title = "Hand tracking and gesture recognition"
tags = ["MediaPipe", "Keypoint detection", "2D", "3D"]
thumbnail = "https://static.rerun.io/hand-tracking-and-gesture-recognition/56d097e347af2a4b7c4649c7d994cc038c02c2f4/480w.png"
thumbnail_dimensions = [480, 480]
-->

Use the [MediaPipe](https://github.com/google-ai-edge/mediapipe/) Hand Landmark and Gesture Detection solutions to
track hands and recognize gestures in images, video, and camera stream.

<picture>
  <img src="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/gesture_detection/2a5a3ec83962623063297fd95de57062372d5db0/1200w.png">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`LineStrips2D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips2d), [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Background
The hand tracking and gesture recognition technology aims to give the ability of the devices to interpret hand movements and gestures as commands or inputs.
At the core of this technology, a pre-trained machine-learning model analyses the visual input and identifies hand landmarks and hand gestures.
The real applications of such technology vary, as hand movements and gestures can be used to control smart devices.
Human-Computer Interaction, Robotics, Gaming, and Augmented Reality are a few of the fields where the potential applications of this technology appear most promising.

In this example, the [MediaPipe](https://developers.google.com/mediapipe/) Gesture and Hand Landmark Detection solutions were utilized to detect and track hand landmarks and recognize gestures.
Rerun was employed to visualize the output of the Mediapipe solution over time to make it easy to analyze the behavior.

## Logging and visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

### Timelines

For each processed video frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time("frame_nr", sequence=frame_idx)
rr.set_time("frame_time", timedelta=1e-9 * frame_time_nano)
```

### Video
The input video is logged as a sequence of [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the `Media/Video` entity.
```python
rr.log(
    "Media/Video",
    rr.Image(frame).compress(jpeg_quality=75)
)
```

### Hand landmark points
Logging the hand landmarks involves specifying connections between the points, extracting pose landmark points and logging them to the Rerun SDK.
The 2D points are visualized over the video and at a separate entity.
Meanwhile, the 3D points allows the creation of a 3D model of the hand for a more comprehensive representation of the hand landmarks.

The 2D and 3D points are logged through a combination of two archetypes.
For the 2D points, the Points2D and LineStrips2D archetypes are utilized. These archetypes help visualize the points and connect them with lines, respectively.
As for the 3D points, the logging process involves two steps. First, a static [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged, that contains the information which maps keypoint ids to labels and how to connect
the keypoints. Defining these connections automatically renders lines between them. Mediapipe provides the `HAND_CONNECTIONS` variable which contains the list of `(from, to)` landmark indices that define the connections.
Second, the actual keypoint positions are logged in 3D [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype.

#### Label mapping and keypoint connections

```python
rr.log(
    "/",
    rr.AnnotationContext(
        rr.ClassDescription(
            info=rr.AnnotationInfo(id=0, label="Hand3D"),
            keypoint_connections=mp.solutions.hands.HAND_CONNECTIONS,
        )
    ),
    static=True,
)

rr.log("Hand3D", rr.ViewCoordinates.LEFT_HAND_Y_DOWN, static=True)
```

#### 2D points

```python
# Log points to the image and Hand entity
for log_key in ["Media/Points", "Hand/Points"]:
    rr.log(
      log_key,
      rr.Points2D(points, radii=10, colors=[255, 0, 0])
    )

# Log connections to the image and Hand entity [128, 128, 128]
for log_key in ["Media/Connections", "Hand/Connections"]:
    rr.log(
      log_key,
      rr.LineStrips2D(np.stack((points1, points2), axis=1), colors=[255, 165, 0])
    )
```

#### 3D points

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

### Detection

To showcase gesture recognition, an image of the corresponding gesture emoji is displayed within a `TextDocument` under the `Detection` entity.

```python
# Log the detection by using the appropriate image
rr.log(
    "Detection",
    rr.TextDocument(f"![Image]({GESTURE_URL + gesture_pic})".strip(), media_type=rr.MediaType.MARKDOWN),
)
```

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/gesture_detection
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m gesture_detection # run the example
```
If you wish to customize it for various videos, adjust the maximum frames, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
$ python -m gesture_detection --help
```
