<!--[metadata]
title = "Live Camera Edge Detection"
tags = ["2D", "canny", "live", "opencv"]
thumbnail = "https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/480w.png"
thumbnail_dimensions = [480, 364]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/1200w.png">
  <img src="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/full.png" alt="Live Camera Edge Detection example screenshot">
</picture>

Visualize the [OpenCV Canny Edge Detection](https://docs.opencv.org/4.x/da/d22/tutorial_py_canny.html) results from a live camera stream.

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image)

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
pip install -r examples/python/live_camera_edge_detection/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/live_camera_edge_detection/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/live_camera_edge_detection/main.py --help 

usage: main.py [-h] [--device DEVICE] [--num-frames NUM_FRAMES] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Streams a local system camera and runs the canny edge detector.

optional arguments:
  -h, --help            show this help message and exit
  --device DEVICE       Which camera device to use. (Passed to `cv2.VideoCapture()`)
  --num-frames NUM_FRAMES
                        The number of frames to log
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer
```

