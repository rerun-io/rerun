---
title: Human Face Tracking
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/human_face_tracking/main.py
tags: [media-pipe, face-tracking, live, camera, 2d, 3d]
---

![mp_face example screenshot](https://static.rerun.io/6efefd1de7c63166c4d99956711b6367206d9835_mp_face.png)

Use the [MediaPipe](https://google.github.io/mediapipe/) Face Detector and Landmarker solutions to detect and track a human face in image, videos, and camera stream.

```bash
pip install -r examples/python/mp_face/requirements.txt
python examples/python/mp_face/main.py
```

## Usage

CLI usage help is available using the `--help` option:

```
$ python examples/python/mp_face/main.py --help
usage: main.py [-h] [--demo-image] [--image IMAGE] [--video VIDEO] [--camera CAMERA] [--max-frame MAX_FRAME] [--max-dim MAX_DIM] [--num-faces NUM_FACES] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE]

Uses the MediaPipe Face Detection to track a human pose in video.

options:
  -h, --help            show this help message and exit
  --demo-image          Run on a demo image automatically downloaded
  --image IMAGE         Run on the provided image
  --video VIDEO         Run on the provided video file.
  --camera CAMERA       Run from the camera stream (parameter is the camera ID, usually 0
  --max-frame MAX_FRAME
                        Stop after processing this many frames. If not specified, will run until interrupted.
  --max-dim MAX_DIM     Resize the image such as its maximum dimension is not larger than this value.
  --num-faces NUM_FACES
                        Max number of faces detected by the landmark model (temporal smoothing is applied only for a value of 1).
  --headless            Don't show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
```

Here is an overview of the options specific to this example:

- *Running modes*: By default, this example streams images from the default webcam. Another webcam can be used by providing a camera index with the `--camera` option. Alternatively, images can be read from a video file (using `--video PATH`) or a single image file (using `--image PATH`). Also, a demo image with two faces can be automatically downloaded and used with `--demo-image`.
- *Max face count*: The maximum face detected by MediaPipe Face Landmarker can be set using `--num-faces NUM`. It defaults to 1, in which case the Landmarker applies temporal smoothing. This parameter doesn't affect MediaPipe Face Detector, which always attempts to detect all faces present in the input images.
- *Image downscaling*: By default, this example logs and runs on the native resolution of the provided images. Input images can be downscaled to a given maximum dimension using `--max-dim DIM`.
- *Limiting frame count*: When running from a webcam or a video file, this example can be set to stop after a given number of frames using `--max-frame MAX_FRAME`. 
