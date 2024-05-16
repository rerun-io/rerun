<!--[metadata]
title = "Live camera edge detection"
tags = ["2D", "Canny", "Live", "OpenCV"]
thumbnail = "https://static.rerun.io/live-camera-edge-detection/f747bcf9ff3039c895f0bf0290e2dea0a72631ea/480w.png"
thumbnail_dimensions = [480, 480]
-->

Visualize the [OpenCV Canny Edge Detection](https://docs.opencv.org/4.x/da/d22/tutorial_py_canny.html) results from a live camera stream.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/1200w.png">
  <img src="https://static.rerun.io/live_camera_edge_detection/bf877bffd225f6c62cae3b87eecbc8e247abb202/full.png" alt="Live Camera Edge Detection example screenshot">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image)

## Background
In this example, the results of the [OpenCV Canny Edge Detection](https://docs.opencv.org/4.x/da/d22/tutorial_py_canny.html) algorithm are visualized.
Canny Edge Detection is a popular edge detection algorithm, and can efficiently extract important structural information from visual objects while notably reducing the computational load.
The process in this example involves converting the input image to RGB, then to grayscale, and finally applying the Canny Edge Detector for precise edge detection.

## Logging and visualizing with Rerun

The visualization in this example were created with the following Rerun code:
### RGB image

The original image is read and logged in RGB format under the entity "image/rgb".
```python
# Log the original image
rgb = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
rr.log("image/rgb", rr.Image(rgb))
```

### Grayscale image

The input image is converted from BGR color space to grayscale, and the resulting grayscale image is logged under the entity "image/gray".
```python
# Convert to grayscale
gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
rr.log("image/gray", rr.Image(gray))
```

### Canny edge detection image

The Canny edge detector is applied to the grayscale image, and the resulting edge-detected image is logged under the entity "image/canny".
```python
# Run the canny edge detector
canny = cv2.Canny(gray, 50, 200)
rr.log("image/canny", rr.Image(canny))
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
pip install -e examples/python/live_camera_edge_detection
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m live_camera_edge_detection # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m live_camera_edge_detection --help
```
