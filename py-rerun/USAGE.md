# Using the `rerun_sdk` Python Library

Install instructions can be found at <https://github.com/rerun-io/rerun#readme>.

## Intro
The Rerun Python SDK is a logging SDK. It lets you log rich data, such as images and point clouds. The logged data is streamed to the Rerun Viewer.

To get started, start the Rerun Viewer by just typing `rerun` in a terminal. It will now wait for the Rerun Python SDK to start sending it log data.

## Logging
Rerun assumes you are using `numpy` for any large chunks of data.

```python
import rerun_sdk as rerun

rerun.connect() # Connect to the separate `rerun` process.

rerun.log_image("rgb_image", image)
```

See more in [`example_dummy.py`](rerun_sdk/examples/example_dummy.py).

## Paths
The first argument to each log function is an _object path_. Each time you log to a specific object path you will update the object, i.e. log a new instance of it along the timeline. Each logging to a path bust be of the same type (you cannot log an image to the same path as a point cloud).

A path can look like this: `detections/object/42/bbox`. Each component (between the slashes) can either be:

* A name (`detections`). Intended for hard-coded names.
* A `"quoted string"`. Intended for things like serial numbers.
* An integer. Intended for hashes or similar.
* A number sequence, prefixed by `#`, intended for indices.
* A UUID.

So for instance, `/foo/bar/#42/5678/"CA426571"/a6a5e96c-fd52-4d21-a394-ffbb6e5def1d` is a valid path.

Example usage:

``` python
for cam in cameras:
    for i, detection in enumerate(cam.detect()):
        rerun.log_rect(f'camera/"{cam.id}"/detection/#{i}', detection.top_left, detection.bottom_right)
```

## Timeless data
The logging functions all have `timeless = False` parameters. Timeless objects belong to all timelines (existing ones, and ones not yet created) and are shown leftmost in the time panel in the viewer. This is useful for object that aren't part of normal data capture, but set the scene for how they are shown. For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be timeless.


## Inline viewer
If you prefer, you can open the viewer directly from Python (blocking the Python process).

To do so, don't call `rerun.connect()`. Instead, call `rerun.show()` at the end of your program and a window will pop up with your logged data.

## Troubleshooting
You can set `RUST_LOG=debug` before running your Python script and/or `rerun` process to get some verbose logging output.
