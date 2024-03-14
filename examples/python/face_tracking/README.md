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


## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`Scalar`](https://www.rerun.io/docs/reference/types/archetypes/scalar) 

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