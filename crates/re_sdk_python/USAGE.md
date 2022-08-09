# Using the `rerun_sdk` Python Library

Install instructions can be found at <https://github.com/rerun-io/rerun#readme>.

## Intro
The Rerun Python SDK is a logging SDK. It lets you log rich data, such as images and point clouds. The logged data is streamed to the Rerun Viewer.

To get started, start the Rerun Viewer by just typing `rerun` in a terminal. It will now wait for the Rerun Python SDK to start sending it log data.

## Logging
Rerun assumes you are using `numpy` for any large chunks of data.

The first argument to each log function is the object name. This needs to be unique for each thing you log. You can not log an image with the same name as a point cloud!

```python
import rerun_sdk as rerun

# Logging an image is easy:
rerun.log_image("rgb_image", image)

# If you are using e.g. `dtype=uin16`, this says that the resolution is 0.1 mm (1m / 10000).
# If you are using `dtype=f64` and you are already using SI units, use `meter=1`.
rerun.log_depth_image("depth_image", depth_image, meter=10_000)

# Log a rectangle (2D bounding box).
# `label` is an optional text that will be shown in the box.
x, y = 200.0, 50.0
w, h = 320, 240
rerun.log_bbox("object", [x, y], [w, h], label="Blue car at 20m")

# Points are logged as an array of positions (2D vs 3D), together with optional colors.
# The positions need to have `shape=[N, 2]` (2D) or `shape=[N, 3]` (3D).
# The colors should be shaped `[N, 4]` (RGBA).
# If you you pass in exactly one color (`shape=[1, 4]`), the same color will be used for all points.
rerun.log_points(f"point_cloud", positions, colors)
```

## Inline viewer
If you prefer, you can open the viewer directly from Python (blocking the Python process).

To do so, call `rerun.buffer()` at the start of you program. This will tell the Rerun SDK to buffer the log data instead of sending it. Then call `rerun.show()` at the end of your program.

## Troubleshooting
You can set `RUST_LOG=debug` before running your Python script to get more verbose output out of the Rerun Logging SDK.
