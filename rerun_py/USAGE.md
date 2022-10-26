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

See more in [`example_car.py`](rerun_sdk/examples/example_car.py).

## Paths
The first argument to each log function is an _object path_. Each time you log to a specific object path you will update the object, i.e. log a new instance of it along the timeline. Each logging to a path bust be of the same type (you cannot log an image to the same path as a point cloud).

A path can look like this: `3d/camera/image/detection/#42/bbox`. Each component (between the slashes) can either be:

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

## Transform hierarchy
The path defines a hierarchy. The root objects all define separate _spaces_. All other objects are by default assumed to be in the same space as its parent object.

Rerun uses the term _space_ to mean _coordinate system_ or _coordinate frame_.

* `world/car` and `world/bike` will be in the same space (same parent)
* `world/car` and `image/detection` will be in different spaces (different root objects)

Objects can be separated into their own spaces by logging special transforms relative to their parents using `rerun.log_rigid3` and `rerun.log_pinhole`. `log_rigid3` is for the camera pose (translation and rotation), while `log_pinhole` is for the camera pinhole projection matrix and image resolution.

Say you have a 3D world with two cameras with known extrinsics (pose) and intrinsics (pinhole model and resolution). You want to log some things in the shared 3D space, and also log each camera image and some detection in these images.

```py
# Log some data to the 3D world:
rerun.log_points("3d/points", …)

# Log first camera:
rerun.log_rigid3("3d/camera/#0", …)
rerun.log_pinhole("3d/camera/#0/image", …)

# Log second camera:
rerun.log_rigid3("3d/camera/#1", …)
rerun.log_pinhole("3d/camera/#1/image", …)

# Log some data to the image spaces of the first camera:
rerun.log_image("3d/camera/#0/image", …)
rerun.log_rect("3d/camera/#0/image/detection", …)
```

Rerun will from this understand out how the `3d` space and the two image spaces (`3d/camera/#0/image` and `3d/camera/#1/image`) relate to each other, allowing you to explore their relationship in the Rerun Viewer. In the 3D view you will see the two cameras show up with their respective camera frustums (based on the intrinsics). If you hover your mouse in one of the image spaces, a corresponding ray will be shot through the 3D space.

Note that none of the names in the path are special.

`rerun.log_rigid3("foo/bar", …)` is logging the relationship between the parent `foo` and the child `bar`,
and `rerun.log_rigid3("foo/bar/baz", …)` is logging the relationship between the parent `bar` and the child `baz`.


## View coordinates
You can use `log_view_coordinates` to set your preferred view coordinate systems.

For camera spaces this can be for instance `rerun.log_view_coordinates("3d/camera", xyz="RDF")` to indicate that `X=Right, Y=Down, Z=Forward`. For convenience, `log_rigid3` also takes this as an argument.
This is required for Rerun to know how to orient your cameras in the 3D view.

For 3D world spaces it can be useful to log what the up-axis is in your coordinate system. This will help Rerun setup a good default view of your 3D scene, as well as make the virtual eye interactions more natural. This can be done with `rerun.log_view_coordinates("3d", up="+Z", timeless=True)`.


## Timeless data
The logging functions all have `timeless = False` parameters. Timeless objects belong to all timelines (existing ones, and ones not yet created) and are shown leftmost in the time panel in the viewer. This is useful for object that aren't part of normal data capture, but set the scene for how they are shown. For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be timeless.


## Inline viewer
If you prefer, you can open the viewer directly from Python (blocking the Python process).

To do so, don't call `rerun.connect()`. Instead, call `rerun.show()` at the end of your program and a window will pop up with your logged data.

## Troubleshooting
You can set `RUST_LOG=debug` before running your Python script and/or `rerun` process to get some verbose logging output.
