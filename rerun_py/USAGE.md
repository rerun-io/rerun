<!--- TODO(emilk): move all of this to https://github.com/rerun-io/rerun-docs/  -->

# Using the `rerun` Python Library

Install instructions can be found at <https://github.com/rerun-io/rerun#readme>.

## Intro
The Rerun Python SDK is a logging SDK. It lets you log rich data, such as images and point clouds. The logged data is streamed to the Rerun Viewer.

## Logging
Rerun assumes you are using `numpy` for any large chunks of data.

```python
import rerun as rr

rr.init("my_app", spawn = True) # Spawn a Rerun Viewer and stream log events to it

rr.log_image("rgb_image", image)
```

Check out the [`examples`](/examples) to see more of how logging can be done in practice.

## Timelines
Each piece of logged data is associated with one or more timelines. By default, each log is added to the `log_time` timeline, with a timestamp assigned by the SDK. Use the _set time_ functions (`set_time_sequence`, `set_time_seconds`, `set_time_nanos`) to associate logs with other timestamps on other timelines.

For example:
```python
for frame in read_sensor_frames():
    rr.set_time_sequence("frame_idx", frame.idx)
    rr.set_time_seconds("sensor_time", frame.timestamp)

    rr.log_points("sensor/points", frame.points)
```
This will add the logged points to the timelines `log_time`, `frame_idx`, and `sensor_time`. You can then choose which timeline you want to organize your data along in the timeline view in the bottom of the Rerun Viewer.

## Paths
The first argument to each log function is an _entity path_. Each time you log to a specific entity path you will update the entity, i.e. log a new instance of it along the timeline. Each logging to a path must be of the same type (you cannot log an image to the same path as a point cloud).

A path can look like this: `world/camera/image/detection/#42/bbox`. Each component (between the slashes) can either be:

* A name (`detections`). Intended for hard-coded names.
* A `"quoted string"`. Intended for things like serial numbers.
* An integer. Intended for hashes or similar.
* A number sequence, prefixed by `#`, intended for indices.
* A UUID.

So for instance, `foo/bar/#42/5678/"CA426571"/a6a5e96c-fd52-4d21-a394-ffbb6e5def1d` is a valid path.

Example usage:

``` python
for cam in cameras:
    for i, detection in enumerate(cam.detect()):
        rr.log_rect(f'camera/"{cam.id}"/detection/#{i}', detection.bbox)
```

## Transform hierarchy
The path defines a hierarchy. The root entities all define separate _spaces_. All other entities are by default assumed to be in the same space as its parent entity.

Rerun uses the term _space_ to mean _coordinate system_ or _coordinate frame_.

* `world/car` and `world/bike` will be in the same space (same parent)
* `world/car` and `image/detection` will be in different spaces (different root entities)

Entities can be separated into their own spaces by logging special transforms relative to their parents using `rr.log_rigid3` and `rr.log_pinhole`. `log_rigid3` is for the camera pose (translation and rotation), while `log_pinhole` is for the camera pinhole projection matrix and image resolution.

Say you have a 3D world with two cameras with known extrinsics (pose) and intrinsics (pinhole model and resolution). You want to log some things in the shared 3D space, and also log each camera image and some detection in these images.

```py
# Log some data to the 3D world:
rr.log_points("world/points", …)

# Log first camera:
rr.log_rigid3("world/camera/#0", parent_from_child=(cam0_pose.pos, cam0_pose.rot))
rr.log_pinhole("world/camera/#0/image", …)

# Log second camera:
rr.log_rigid3("world/camera/#1", parent_from_child=(cam1_pose.pos, cam1_pose.rot))
rr.log_pinhole("world/camera/#1/image", …)

# Log some data to the image spaces of the first camera:
rr.log_image("world/camera/#0/image", …)
rr.log_rect("world/camera/#0/image/detection", …)
```

Rerun will from this understand how the `world` space and the two image spaces (`world/camera/#0/image` and `world/camera/#1/image`) relate to each other, which allows you to explore their relationship in the Rerun Viewer. In the 3D view you will see the two cameras show up with their respective camera frustums (based on the intrinsics). If you hover your mouse in one of the image spaces, a corresponding ray will be shot through the 3D space. In the future Rerun will also be able to transform entities between spaces, so that you can view 3D entities projected onto a 2D space, for instance.

Note that none of the names in the path are special.

`rr.log_rigid3("foo/bar", …)` is logging the relationship between the parent `foo` and the child `bar`,
and `rr.log_rigid3("foo/bar/baz", …)` is logging the relationship between the parent `bar` and the child `baz`.

### Unknown transforms
Sometimes you have a child space that doesn't have an identity transform to the parent, but you don't know the transform, or don't know it yet.
You can use `rr.log_unknown_transform("parent/child")` to indicate to that `child` is separate from `parent`. You can later replace this unknown transform with a known one, using e.g. `log_rigid`.


## View coordinates
You can use `log_view_coordinates` to set your preferred view coordinate systems.

Each entity defines its own coordinate system, called a space.
By logging view coordinates you can give semantic meaning to the XYZ axes of the space.
This is for instance useful for camera entities ("what axis is forward?").

For camera spaces this could for instance be `rr.log_view_coordinates("world/camera", xyz="RDF")` to indicate that `X=Right, Y=Down, Z=Forward`. For convenience, `log_rigid3` also takes this as an argument. Logging view coordinates helps Rerun figure out how to interpret your logged camera.

For 3D world spaces it can be useful to log what the up-axis is in your coordinate system. This will help Rerun set a good default view of your 3D scene, as well as make the virtual eye interactions more natural. This can be done with `rr.log_view_coordinates("world", up="+Z", timeless=True)`.


## Timeless data
The logging functions all have `timeless = False` parameters. Timeless entities belong to all timelines (existing ones, and ones not yet created) and are shown leftmost in the time panel in the viewer. This is useful for entity that aren't part of normal data capture, but set the scene for how they are shown. For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be timeless.


## Inline viewer
If you prefer, you can open the viewer directly from Python (blocking the Python process).

To do so, don't call `rr.connect()`. Instead, call `rr.show()` at the end of your program and a window will pop up with your logged data.

## Troubleshooting
You can set `RUST_LOG=debug` before running your Python script and/or `rerun` process to get some verbose logging output.

# Help
Most documentation is found in the docstrings of the functions in the Rerun. Either check out the docstrings directly in code or use the built in `help()` function. For example, to see the docstring of the `log_image` function, open a python terminal and run:

```python
import rerun as rr
help(rr.log_image)
```

## Bounded memory use

You can set `--memory-limit=16GB` to tell the Rerun Viewer to purge older log data when memory use goes above that limit. This is useful for using Rerun in _continuous_ mode, i.e. where you keep logging new data to Rerun forever.

It is still possible to log data faster than the Rerun Viewer can process it, and in those cases you may still run out of memory unless you also set `--drop-at-latency=200ms` or similar.
