<!--[metadata]
title = "Human Pose Tracking"
tags = ["mediapipe", "keypoint-detection", "2D", "3D"]
description = "Use the MediaPipe Pose solution to detect and track a human pose in video."
thumbnail = "https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/480w.png"
thumbnail_dimensions = [480, 272]
channel = "main"
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/1200w.png">
  <img src="https://static.rerun.io/human_pose_tracking/37d47fe7e3476513f9f58c38da515e2cd4a093f9/full.png" alt="">
</picture>

Use the [MediaPipe Pose Landmark Detection](https://developers.google.com/mediapipe/solutions/vision/pose_landmarker) solution to detect and track a human pose in video.



## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image)

## Background
The [MediaPipe Pose Landmark Detection](https://developers.google.com/mediapipe/solutions/vision/pose_landmarker) solution detects and tracks human pose landmarks and produces segmentation masks for humans. The solution targets real-time inference on video streams. In this example we use Rerun to visualize the output of the Mediapipe solution over time to make it easy to analyze the behavior.


# Logging and Visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

## Timelines

For each processed video frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time_seconds("time", bgr_frame.time)
rr.set_time_sequence("frame_idx", bgr_frame.idx)
```

## Video
The input video is logged as a sequence of 
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the 'Video' entity.
```python
rr.log(
    "video/rgb", 
    rr.Image(rgb).compress(jpeg_quality=75)
)
```

## Segmentation Mask

The segmentation result is logged through a combination of two archetypes. The segmentation
image itself is logged as an 
[`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) and
contains the id for each pixel. The color is determined by the
[`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) which is
logged with `timeless=True` as it should apply to the whole sequence.

### Label Mapping

```python
rr.log(
        "video/mask",
        rr.AnnotationContext(
            [
                rr.AnnotationInfo(id=0, label="Background"),
                rr.AnnotationInfo(id=1, label="Person", color=(0, 0, 0)),
            ]
        ),
        timeless=True,
    )
```

### Segmentation Image

```python
rr.log(
    "video/mask", 
    rr.SegmentationImage(segmentation_mask.astype(np.uint8))
)
```

## Body Pose Points
Logging the body pose landmarks involves specifying connections between the points, extracting pose landmark points and logging them to the Rerun SDK.
The 2D points are visualized over the image/video for a better understanding and visualization of the body pose. The 3D points allows the creation of a 3D model of the body posture for a more comprehensive representation of the human pose.



The 2D and 3D points are logged through a combination of two archetypes. First, a timeless
[`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged, that contains the information which maps keypoint ids to labels and how to connect
the keypoints. In both 2D and 3D points, specifying connections between points is essential. 
Defining these connections automatically renders lines between them. Using the information provided by MediaPipe, 
you can get the pose points connections from the `POSE_CONNECTIONS` set. Second, the actual keypoint positions are logged in 2D
nd 3D as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d) and
[`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetypes, respectively. 

### Label Mapping and Keypoint Connections

```python
rr.log(
    "/",
    rr.AnnotationContext(
        rr.ClassDescription(
            info=rr.AnnotationInfo(id=1, label="Person"),
            keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
            keypoint_connections=mp_pose.POSE_CONNECTIONS,
        )
    ),
    timeless=True,
)
```

### 2D Points

```python
rr.log(
    "video/pose/points", 
    rr.Points2D(landmark_positions_2d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark)
)
```

### 3D Points

```python
rr.log(
    "person/pose/points",
    rr.Points3D(landmark_positions_3d, class_ids=1, keypoint_ids=mp_pose.PoseLandmark),
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
pip install -r examples/python/human_pose_tracking/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/human_pose_tracking/main.py # run the example
```

If you wish to customize it for various videos, adjust the maximum frames, or explore additional features, use the CLI with the `--help` option for guidance:

```bash
python examples/python/human_pose_tracking/main.py --help 
```