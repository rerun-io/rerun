<!--[metadata]
title = "Face Tracking"
tags = ["2D", "3D", "camera", "face-tracking", "live", "mediapipe", "time-series"]
thumbnail = "https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/480w.png"
thumbnail_dimensions = [480, 335]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/1200w.png">
  <img src="https://static.rerun.io/mp_face/f5ee03278408bf8277789b637857d5a4fda7eba3/full.png" alt="screenshot of the Rerun visualization of the MediaPipe Face Detector and Landmarker">
</picture>


Use the [MediaPipe](https://google.github.io/mediapipe/) Face Detector and Landmarker solutions to detect and track a human face in image, videos, and camera stream.


# Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`Scalar`](https://www.rerun.io/docs/reference/types/archetypes/scalar) 

# Logging and Visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

## Timelines

For each processed video frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time_seconds("time", bgr_frame.time)
rr.set_time_sequence("frame_idx", bgr_frame.idx)
```

## Video
The input video is logged as a sequence of [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the 'Video' entity.
```python
rr.log(
    "video/image", 
    rr.Image(frame).compress(jpeg_quality=75)
)
```

## Face Landmark Points
Logging the face landmarks involves specifying connections between the points, extracting face landmark points and logging them to the Rerun SDK. 
The 2D points are visualized over the video/image for a better understanding and visualization of the face. 
The 3D points allows the creation of a 3D model of the face reconstruction for a more comprehensive representation of the face.

The 2D and 3D points are logged through a combination of two archetypes. First, a timeless
[`ClassDescription`](https://www.rerun.io/docs/reference/types/datatypes/class_description) is logged, that contains the information which maps keypoint ids to labels and how to connect
the keypoints. Defining these connections automatically renders lines between them. 
Second, the actual keypoint positions are logged in 2D and 3D as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d) and [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetypes, respectively.

### Label Mapping and Keypoint Connections

An annotation context is logged with one class ID assigned per facial feature. The class description includes the connections between corresponding keypoints extracted from the MediaPipe face mesh solution. 
A class ID array is generated to match the class IDs in the annotation context with keypoint indices (to be utilized as the class_ids argument to rr.log).
```python
# Initialize a list of facial feature classes from MediaPipe face mesh solution
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

# Initialize class descriptions and class IDs array
self._class_ids = [0] * mp.solutions.face_mesh.FACEMESH_NUM_LANDMARKS_WITH_IRISES
class_descriptions = []

# Loop through each facial feature class
for i, klass in enumerate(classes):
    # MediaPipe only provides connections for class, not actual class per keypoint. So we have to extract the
    # classes from the connections.
    ids = set()
    for connection in klass:
        ids.add(connection[0])
        ids.add(connection[1])

    for id_ in ids:
        self._class_ids[id_] = i

    # Append class description with class ID and keypoint connections
    class_descriptions.append(
        rr.ClassDescription(
            info=rr.AnnotationInfo(id=i),
            keypoint_connections=klass,
        )
    )

# Log annotation context for video/landmarker and reconstruction entities
rr.log("video/landmarker", rr.AnnotationContext(class_descriptions), timeless=True)
rr.log("reconstruction", rr.AnnotationContext(class_descriptions), timeless=True)

rr.log("reconstruction", rr.ViewCoordinates.RDF, timeless=True) # properly align the 3D face in the viewer
```

With the below annotation, the keypoints will be connected with lines to enhance visibility in the `video/detector` entity.
```python
rr.log(
    "video/detector",
    rr.ClassDescription(
        info=rr.AnnotationInfo(id=0), keypoint_connections=[(0, 1), (1, 2), (2, 0), (2, 3), (0, 4), (1, 5)]
    ),
    timeless=True,
)
```
### Bounding Box

```python
rr.log(
    f"video/detector/faces/{i}/bbox",
    rr.Boxes2D(
        array=[bbox.origin_x, bbox.origin_y, bbox.width, bbox.height], array_format=rr.Box2DFormat.XYWH
    ),
    rr.AnyValues(index=index, score=score),
)
```


### 2D Points

```python
rr.log(
    f"video/detector/faces/{i}/keypoints", 
    rr.Points2D(pts, radii=3, keypoint_ids=list(range(6)))
)
```

```python
rr.log(
    f"video/landmarker/faces/{i}/landmarks",
    rr.Points2D(pts, radii=3, keypoint_ids=keypoint_ids, class_ids=self._class_ids),
)
```

### 3D Points

```python
rr.log(
    f"reconstruction/faces/{i}",
    rr.Points3D(
        [(lm.x, lm.y, lm.z) for lm in landmark],
        keypoint_ids=keypoint_ids,
        class_ids=self._class_ids,
    ),
)
```

## Scalar 
Blendshapes are essentially predefined facial expressions or configurations that can be detected by the face landmark detection model. Each blendshape typically corresponds to a specific facial movement or expression, such as blinking, squinting, smiling, etc.

The blendshapes are logged along with their corresponding scores.
```python
for blendshape in blendshapes:
    if blendshape.category_name in BLENDSHAPES_CATEGORIES:
        rr.log(f"blendshapes/{i}/{blendshape.category_name}", rr.Scalar(blendshape.score))
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
pip install -r examples/python/face_tracking/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/face_tracking/main.py # run the example
```
If you wish to customize it for various videos, adjust the maximum frames, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/face_tracking/main.py --help 

usage: main.py [-h] [--demo-image] [--image IMAGE] [--video VIDEO] [--camera CAMERA] [--max-frame MAX_FRAME] [--max-dim MAX_DIM] [--num-faces NUM_FACES] [--headless]
               [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Uses the MediaPipe Face Detection to track a human pose in video.

optional arguments:
  -h, --help            show this help message and exit
  --demo-image          Run on a demo image automatically downloaded
  --image IMAGE         Run on the provided image
  --video VIDEO         Run on the provided video file.
  --camera CAMERA       Run from the camera stream (parameter is the camera ID, usually 0)
  --max-frame MAX_FRAME
                        Stop after processing this many frames. If not specified, will run until interrupted.
  --max-dim MAX_DIM     Resize the image such as its maximum dimension is not larger than this value.
  --num-faces NUM_FACES
                        Max number of faces detected by the landmark model (temporal smoothing is applied only for a value of 1).
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer
```