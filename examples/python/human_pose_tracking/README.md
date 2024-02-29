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

# Overview

Use the [MediaPipe](https://google.github.io/mediapipe/)  Pose Landmark Detection solutions to track human body pose in images and videos.

Logging Details:

1. Human Body Pose Landmarks as 2D Points:

   - Specify connections between the points

   - Extracts hand landmark points as 2D image coordinates.

   - Logs the 2D points to the Rerun SDK.
   

2. Human Body Pose Landmarks as 3D Points:

    - Specify connections between the points

    - Detects body pose landmarks using MediaPipe solutions.

    - Converts the detected body pose landmarks into 3D coordinates.

    - Logs the 3D points to the Rerun SDK.

# Run

```bash
pip install -r examples/python/human_pose_tracking/requirements.txt
python examples/python/human_pose_tracking/main.py
```
# Usage

CLI usage help is available using the `--help` option:

```bash
$ python examples/python/gesture_detection/main.py --help
usage: main.py [-h] [--video {backflip,soccer}] [--dataset-dir DATASET_DIR] [--video-path VIDEO_PATH] [--no-segment] [--max-frame MAX_FRAME] [--headless] [--connect]
               [--serve] [--addr ADDR] [--save SAVE] [-o]

Uses the MediaPipe Pose solution to track a human pose in video.

optional arguments:
  -h, --help            show this help message and exit
  --video {backflip,soccer}
                        The example video to run on.
  --dataset-dir DATASET_DIR
                        Directory to save example videos to.
  --video-path VIDEO_PATH
                        Full path to video to run on. Overrides `--video`.
  --no-segment          Don t run person segmentation.
  --max-frame MAX_FRAME
                        Stop after processing this many frames. If not specified, will run until interrupted.
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer

```

# Logging Data

## Timelines for Video

You can utilize Rerun timelines' functions to associate data with one or more timelines. 
As a result, each frame of the video can be linked with its corresponding timestamp.
Here is achieved using the  `set_time_seconds`  and  `set_time_sequence`  Rerun's functions.

```python

def track_pose(video_path: str, *, segment: bool, max_frame_count: int | None) -> None:
    mp_pose = mp.solutions.pose

    # ... existing code ...

    with closing(VideoSource(video_path)) as video_source, mp_pose.Pose(enable_segmentation=segment) as pose:
        for idx, bgr_frame in enumerate(video_source.stream_bgr()):
            if max_frame_count is not None and idx >= max_frame_count:
                break

            rgb = cv2.cvtColor(bgr_frame.data, cv2.COLOR_BGR2RGB)
            
            # Associate frame with the data
            rr.set_time_seconds("time", bgr_frame.time)
            rr.set_time_sequence("frame_idx", bgr_frame.idx)
            
            # ... logging data ...

```

## Keypoint-Connections

The class description contains the information which maps keypoint ids to labels and how to connect
the keypoints to a skeleton. In both 2D and 3D points, specifying connections between points is essential. 
Defining these connections automatically renders lines between them. Using the information provided by MediaPipe, 
you can get the pose points connections from the `POSE_CONNECTIONS` set.

```python
def track_pose(video_path: str, *, segment: bool, max_frame_count: int | None) -> None:
    mp_pose = mp.solutions.pose

    rr.log(
        "/",
        rr.AnnotationContext(
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=0, label="Person"),
                keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
                keypoint_connections=mp_pose.POSE_CONNECTIONS,
            )
        ),
        timeless=True,
    )
```

A timeless
[`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged (note, that
this is equivalent to logging an
[`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) as in the
segmentation case). 

## Body Pose Landmarks as 2D Points

[![Body Pose Landmarks_2d_points](https://github.com/rerun-io/rerun/assets/49308613/d5f1b3b5-c55e-44a3-8ad2-6e425ae8e627)](https://github.com/rerun-io/rerun/assets/49308613/b7154548-f5ab-4371-b677-0c902404630f)

You can extract body pose landmark points as image coordinates. 
These coordinates are then logged as 2D points using the  type [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d).

```python
def track_pose(video_path: str, *, segment: bool, max_frame_count: int | None) -> None:
    mp_pose = mp.solutions.pose
    
    rr.log(
        "/",
        rr.AnnotationContext(
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=0, label="Person"),
                keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
                keypoint_connections=mp_pose.POSE_CONNECTIONS,
            )
        ),
        timeless=True,
    )

    with closing(VideoSource(video_path)) as video_source, mp_pose.Pose(enable_segmentation=segment) as pose:
        for idx, bgr_frame in enumerate(video_source.stream_bgr()):
            if max_frame_count is not None and idx >= max_frame_count:
                break

            rgb = cv2.cvtColor(bgr_frame.data, cv2.COLOR_BGR2RGB)
            
            # Associate frame with the data
            rr.set_time_seconds("time", bgr_frame.time)
            rr.set_time_sequence("frame_idx", bgr_frame.idx)
            
            # Present the video
            rr.log("video/rgb", rr.Image(rgb).compress(jpeg_quality=75))

            # Get the prediction results
            results = pose.process(rgb)
            h, w, _ = rgb.shape
            
            # Log 2d points to 'video' entity
            landmark_positions_2d = read_landmark_positions_2d(results, w, h)
            if landmark_positions_2d is not None:
                rr.log(
                    "video/pose/points",
                    rr.Points2D(landmark_positions_2d, class_ids=0, keypoint_ids=mp_pose.PoseLandmark),
                )
```
## Body Pose Landmarks as 3D Points

[![Body Pose Landmarks_2d_points](https://github.com/rerun-io/rerun/assets/49308613/f88e9774-fbf4-4aea-9c1c-d9162df09a53)](https://github.com/rerun-io/rerun/assets/49308613/8268b398-07f7-4e2f-bc91-992bcaf2d850)

You can first define the connections between the points using keypoints from [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) in the init function,
and then log them as 3D points using the [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) type.

```python

def track_pose(video_path: str, *, segment: bool, max_frame_count: int | None) -> None:
    mp_pose = mp.solutions.pose

    rr.log(
        "/",
        rr.AnnotationContext(
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=0, label="Person"),
                keypoint_annotations=[rr.AnnotationInfo(id=lm.value, label=lm.name) for lm in mp_pose.PoseLandmark],
                keypoint_connections=mp_pose.POSE_CONNECTIONS,
            )
        ),
        timeless=True,
    )

    rr.log("person", rr.ViewCoordinates.RIGHT_HAND_Y_DOWN, timeless=True)

    with closing(VideoSource(video_path)) as video_source, mp_pose.Pose(enable_segmentation=segment) as pose:
        for idx, bgr_frame in enumerate(video_source.stream_bgr()):
            if max_frame_count is not None and idx >= max_frame_count:
                break

            rgb = cv2.cvtColor(bgr_frame.data, cv2.COLOR_BGR2RGB)
            
            # Associate frame with the data
            rr.set_time_seconds("time", bgr_frame.time)
            rr.set_time_sequence("frame_idx", bgr_frame.idx)
            
            # Present the video
            rr.log("video/rgb", rr.Image(rgb).compress(jpeg_quality=75))

            # Get the prediction results
            results = pose.process(rgb)
            h, w, _ = rgb.shape
            
            # New entity "Person" for the 3D presentation
            landmark_positions_3d = read_landmark_positions_3d(results)
            if landmark_positions_3d is not None:
                rr.log(
                    "person/pose/points",
                    rr.Points3D(landmark_positions_3d, class_ids=0, keypoint_ids=mp_pose.PoseLandmark),
                )

```
