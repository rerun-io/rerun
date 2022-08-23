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

rerun.connect() # Connect to the separate `rerun` process.

rerun.log_image("rgb_image", image)
```

See more in [`example.py`](./example.py).

## Inline viewer
If you prefer, you can open the viewer directly from Python (blocking the Python process).

To do so, don't call `rerun.connect()`. Instead, call `rerun.show()` at the end of your program and a window will pop up with your logged data.

## Troubleshooting
You can set `RUST_LOG=debug` before running your Python script and/or `rerun` process to get some verbose logging output.
