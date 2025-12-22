<!--[metadata]
title = "Human pose tracking"
tags = ["MediaPipe", "Keypoint detection", "2D", "3D"]
thumbnail = "https://static.rerun.io/human-pose-tracking/5d62a38b48bed1467698d4dc95c1f9fba786d254/480w.png"
thumbnail_dimensions = [480, 480]
-->

Use the [MediaPipe Pose Landmark Detection](https://developers.google.com/mediapipe/solutions/vision/pose_landmarker) solution to detect and track a human pose in video.

<picture data-inline-viewer="examples/human_pose_tracking">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/1200w.png">
  <img src="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/full.png" alt="">
</picture>

## Used Rerun types

[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image)

## Background

Human pose tracking is a task in computer vision that focuses on identifying key body locations, analyzing posture, and categorizing movements.
At the heart of this technology is a pre-trained machine-learning model to assess the visual input and recognize landmarks on the body in both image coordinates and 3D world coordinates.
The use cases and applications of this technology include but are not limited to Human-Computer Interaction, Sports Analysis, Gaming, Virtual Reality, Augmented Reality, Health, etc.

In this example, the [MediaPipe Pose Landmark Detection](https://developers.google.com/mediapipe/solutions/vision/pose_landmarker) solution was utilized to detect and track human pose landmarks and produces segmentation masks for humans.
Rerun was employed to visualize the output of the Mediapipe solution over time to make it easy to analyze the behavior.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code.

### Timelines

For each processed video frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time("time", duration=bgr_frame.time)
rr.set_time("frame_idx", sequence=bgr_frame.idx)
```

### Video

The input video is logged as a sequence of
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the 'Video' entity.

```python
rr.log(
    "video/rgb",
    rr.Image(rgb).compress(jpeg_quality=75)
)
```

### Segmentation mask

The segmentation result is logged through a combination of two archetypes. The segmentation
image itself is logged as a
[`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) and
contains the id for each pixel. The color is determined by the
[`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) which is
logged with `static=True` as it should apply to the whole sequence.

#### Label mapping

```python
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
```

#### Segmentation image

```python
rr.log("video/mask", rr.SegmentationImage(binary_segmentation_mask.astype(np.uint8)))
```

### Body pose points

Logging the body pose as a skeleton involves specifying the connectivity of its keypoints (i.e., pose landmarks), extracting the pose landmarks, and logging them as points to Rerun. In this example, both the 2D and 3D estimates from Mediapipe are visualized.

The skeletons are logged through a combination of two archetypes. First, a static
[`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged, that contains the information which maps keypoint ids to labels and how to connect
the keypoints. By defining these connections Rerun will automatically add lines between them. Mediapipe provides the `POSE_CONNECTIONS` variable which contains the list of `(from, to)` landmark indices that define the connections. Second, the actual keypoint positions are logged in 2D
and 3D as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d) and
[`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetypes, respectively.

#### Label mapping and keypoint connections

```python
rr.log(
    "/",
    rr.AnnotationContext(
        rr.ClassDescription(
            info=rr.AnnotationInfo(id=1, label="Person"),
            keypoint_annotations=[
                rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark
            ],
            keypoint_connections=mp_pose.POSE_CONNECTIONS,
        )
    ),
    static=True,
)
```

#### 2D points

```python
rr.log(
    "video/pose/points",
    rr.Points2D(landmark_positions_2d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark)
)
```

#### 3D points

```python
rr.log(
    "person/pose/points",
    rr.Points3D(landmark_positions_3d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark),
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
pip install -e examples/python/human_pose_tracking
```

To experiment with the provided example, simply execute the main Python script:

```bash
python -m human_pose_tracking # run the example
```

If you wish to customize it for various videos, adjust the maximum frames, or explore additional features, use the CLI with the `--help` option for guidance:

```bash
python -m human_pose_tracking --help
```
