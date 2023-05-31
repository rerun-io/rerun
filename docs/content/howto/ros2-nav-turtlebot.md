---
title: Using Rerun with ROS 2
order: 2
ogImageUrl: /docs-media/og-howto-ros.jpg
description: Rerun does not yet have native ROS support, but many of the concepts in ROS and Rerun line up fairly well. In this guide, you will learn how to write a simple ROS 2 python node that subscribes to some common ROS topics and logs them to Rerun. 
---

Rerun does not yet have native ROS support, but many of the concepts in ROS and Rerun 
line up fairly well. In this guide, you will learn how to write a simple ROS 2 python node
that subscribes to some common ROS topics and logs them to Rerun.

For information on future plans to enable more native ROS support
see [#1537](https://github.com/rerun-io/rerun/issues/1537)

The following is primarily intended for existing ROS 2 users. It will not spend much time
covering how to use ROS 2 itself.  If you are a Rerun user that is curious about ROS,
please consult the [ROS 2 Documentation](https://docs.ros.org/en/humble/index.html) instead.

All of the code for this guide can be found on Github in
[rerun/examples/python/ros](https://github.com/rerun-io/rerun/blob/main/examples/python/ros/).

![Rerun 3D view of ROS 2 turtlebot3 navigation demo](/docs-media/ros1_preview.png)

## Prerequisites

Other relevant tutorials:
 - [Logging with Python](../getting-started/logging-python.md)
 - [Viewer Walkthrough](../getting-started/viewer-walkthrough.md)

### ROS 2 & navigation
You will need to have installed [ROS 2 Humble Hawksbill](https://docs.ros.org/en/humble/index.html)
and the [turtlebot3 navigation getting-started example](https://navigation.ros.org/getting_started/index.html).

Installing ROS is outside the scope of this guide, but you will need the equivalent of the following packages:
```bash
$ sudo apt install ros-humble-desktop gazebo ros-humble-navigation2 ros-humble-turtlebot3 ros-humble-turtlebot3-gazebo
```
If you don't already have ROS installed, we recommend trying [RoboStack](https://robostack.github.io/) for setting up your installation.

Before proceeding, you should follow the [navigation example](https://navigation.ros.org/getting_started/index.html)
and confirm that you can successfully run:
```bash
$ export TURTLEBOT3_MODEL=waffle
$ export GAZEBO_MODEL_PATH=/opt/ros/humble/share/turtlebot3_gazebo/models
$ ros2 launch nav2_bringup tb3_simulation_launch.py headless:=False
```

Make sure that you can set the 2D Pose Estimate and send a Navigation Goal via rviz. You can now leave this
running in the background for the remainder of the guide.

### Additional Dependencies

The code for this guide is in the `rerun` repository. If you do not already have rerun cloned,
you should do so now:
```bash
git clone git@github.com:rerun-io/rerun.git
cd rerun
```
The example code can be found in the folder: `examples/python/ros`.

In addition to the ROS dependencies, the Rerun node makes use of some dependencies specified in
[`requirements.txt`](https://github.com/rerun-io/rerun/blob/main/examples/python/ros/requirements.txt).

Rerun recommends using `venv` (or the equivalent) to create an environment for installing these
dependencies.  Note that *after* setting up your virtualenv you will need to activate your ROS2
environment.
```bash
$ python3 -m venv venv
$ source venv/bin/active
(venv) $ pip install -r examples/python/ros/requirements.txt
(venv) $ source /opt/ros/humble/setup.bash
```

## Running the example

With the previous dependencies installed, and gazebo running, you should now be able to launch the Rerun ROS example:
```bash
(venv) $ python3 examples/python/ros/main.py
```

You should see a window similar to:
![Initial window layout of Rerun 3D view of ROS 2 turtlebot3 navigation demo](/docs-media/ros2_launched.png)

Use rviz to send a new navigation goal and confirm that rerun updates with new data as turtlebot drives around
the environment.

### Overview

If you are familiar with the turtlebot nav example and rviz, this view will likely be familiar:

 * `map/box` is a placeholder for the map. (This will eventually be a map: [#1531](https://github.com/rerun-io/rerun/issues/1531).)
 * `map/robot` is a transform representing the robot pose logged as a [rigid3 transform](../reference/primitives.md#transform).
 * `map/robot/urdf` contains the `URDF` logged as a [mesh3d](../reference/primitives.md#mesh).
 * `map/robot/scan` contains a `LaserScan` msg logged as a [linestrip3d](../reference/primitives.md#line-3d). (This will eventually be a
   native type: [#1534](https://github.com/rerun-io/rerun/issues/1534).)
 * `map/robot/camera` contains a `CameraInfo` msg logged as a [pinhole transform](../reference/primitives.md#transform).
 * `map/robot/camera/img` contains an `Image` msg logged as an [image](../reference/primitives.md#tensors--images).
 * `map/robot/camera/points` contains a `PointCloud2` msg logged as a [point3d batch](../reference/primitives.md#point-3d).
 * `map/points` contains a second copy of `PointCloud2` with a different transform.  (This is a workaround until Rerun
   has support for ROS-style fixed frames [#1522](https://github.com/rerun-io/rerun/issues/1522).)
 * `odometry/vel` is a plot of the linear velocity of the robot.
 * `odometry/ang_vel` is a plot of the angular velocity of the robot.

## Code Explanation

It may be helpful to open [rerun/examples/python/ros/main.py](https://github.com/rerun-io/rerun/blob/main/examples/python/ros/main.py)
to follow along.

Outside of TF, the node is mostly stateless. At a very high level, for each ROS message we are interested in, we create a
subscriber with a callback that does some form of data transformation and then logs the data to Rerun. 

For simplicity, this example uses the rosclpy `MultiThreadedExecutor` and `ReentrantCallbackGroup` for each topic. This
allows each callback thread to do TF lookups without blocking the other incoming messages. More advanced ROS execution
models and using asynchronous TF lookups are outside the scope of this guide.

### Updating Time

First of all, we want our messages to show up on the timeline based on their *stamped* time rather than the
time that they were received by the listener, or relayed to Rerun.

To do this, we will use a Rerun timeline called `ros_time`.

Each callback follows a common pattern of updating `ros_time` based on the stamped time of the message that was
received.
```python
def some_msg_callback(self, msg: Msg):
    time = Time.from_msg(msg.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)
```
This timestamp will apply to all subsequent log calls on in this callback (on this thread) until the time is updated
again.


### TF to `log_rigid3`
Next, we need to map the [ROS TF2](https://docs.ros.org/en/humble/Concepts/About-Tf2.html) transforms to the
corresponding [Rerun Transforms](../concepts/spaces-and-transforms.md#space-transformations).

In Rerun, each path represents a coordinate frame, so we need to decide which TF frame each path will
correspond to. In general, this is the frame_id of the sensor data that will be logged to that
path. For consistency, we define this once in `__init__()`.
```python
# Define a mapping for transforms
self.path_to_frame = {
    "map": "map",
    "map/points": "camera_depth_frame",
    "map/robot": "base_footprint",
    "map/robot/scan": "base_scan",
    "map/robot/camera": "camera_rgb_optical_frame",
    "map/robot/camera/points": "camera_depth_frame",
}
```

Because we have chosen a Rerun path hierarchy that does not exactly match the TF graph topology
the values for the transforms at these paths need to be derived using TF on the client logging
side at log-time.  In the future, Rerun will support deriving these values in the viewer
(see: [#1533](https://github.com/rerun-io/rerun/issues/1533) for more details), allowing all
of this code to go away.

For now, on each incoming log message, we want to use the mapping to update the transform
at the timestamp in question:
```python
def log_tf_as_rigid3(self, path: str, time: Time) -> None:
    """Helper to look up a transform with tf and log using `log_rigid3`."""
    # Get the parent path
    parent_path = path.rsplit("/", 1)[0]

    # Find the corresponding frames from the mapping
    child_frame = self.path_to_frame[path]
    parent_frame = self.path_to_frame[parent_path]

    # Do the TF lookup to get transform from child (source) -> parent (target)
    try:
        tf = self.tf_buffer.lookup_transform(parent_frame,
                                             child_frame,
                                             time,
                                             timeout=Duration(seconds=0.1))
        t = tf.transform.translation
        q = tf.transform.rotation
        rr.log_rigid3(path, parent_from_child=([t.x, t.y, t.z],
                                               [q.x, q.y, q.z, q.w]))
    except TransformException as ex:
        print("Failed to get transform: {}".format(ex))
```

As an example of logging points in the map frame, we simply call:
```python
rr.log_points("map/points", positions=pts, colors=colors)
self.log_tf_as_rigid3("map/points", time)
```

Note that because we previously called `set_time_nanos` in this callback, this transform will
be logged to the same point on the timeline as the data, using a timestamp looked up from TF at the
matching timepoint.

### Odometry to `log_scalar` and `log_rigid3`
When receiving odometry messages, we log the linear and angular velocities using `rr.log_scalar`.
Additionally, since we know that odometry will also update the `map/robot` transform, we use
this as a cue to look up the corresponding transform and log it.
```
def odom_callback(self, odom: Odometry) -> None:
    """Update transforms when odom is updated."""
    time = Time.from_msg(odom.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)

    # Capture time-series data for the linear and angular velocities
    rr.log_scalar("odometry/vel", odom.twist.twist.linear.x)
    rr.log_scalar("odometry/ang_vel", odom.twist.twist.angular.z)

    # Update the robot pose itself via TF
    self.log_tf_as_rigid3("map/robot", time)
```

### CameraInfo to `log_pinhole`
Not all Transforms are rigid as defined in TF. The other transform we want to log
is the pinhole projection that is stored in the `CameraInfo` msg.

Fortunately, the `image_geometry` package has a `PinholeCameraModel` that exposes
the intrinsic matrix in the same structure used by Rerun `rr.log_pinhole`:
```python
def __init__(self) -> None:
    # ...
    self.model = PinholeCameraModel()

def cam_info_callback(self, info: CameraInfo) -> None:
    """Log a `CameraInfo` with `log_pinhole`."""
    time = Time.from_msg(info.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)

    self.model.fromCameraInfo(info)

    rr.log_pinhole(
        "map/robot/camera/img",
        child_from_parent=self.model.intrinsicMatrix(),
        width=self.model.width,
        height=self.model.height,
    )
```

### Image to `log_image`
ROS Images can also be mapped to Rerun very easily, using the `cv_bridge` package.
The output of `cv_bridge.imgmsg_to_cv2` can be fed directly into `rr.log_image`:
```python
def __init__(self) -> None:
    # ...
    self.cv_bridge = cv_bridge.CvBridge()

def image_callback(self, img: Image) -> None:
    """Log an `Image` with `log_image` using `cv_bridge`."""
    time = Time.from_msg(img.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)

    rr.log_image("map/robot/camera/img", self.cv_bridge.imgmsg_to_cv2(img))
    self.log_tf_as_rigid3("map/robot/camera", time)
```

### PointCloud2 to `log_points`
The ROS [PointCloud2](https://github.com/ros2/common_interfaces/blob/humble/sensor_msgs/msg/PointCloud2.msg) message
is stored as a binary blob that needs to be reinterpreted using the details about its fields. Each field is
a named collection of offsets into the data buffer, and datatypes. The `sensor_msgs_py` package includes a `point_cloud2`
reader, which can be used to convert to a Rerun-compatible numpy array.

These fields are initially returned as numpy structured arrays, whereas Rerun currently expects an unstructured
array of Nx3 floats.

Color is extracted in a similar way, although the realsense gazebo driver does not provide the correct offsets for
the r,g,b channels, requiring us to patch the field values.

After extracting the positions and colors as numpy arrays, the entire cloud can be logged as a batch with `rr.log_points`
```python
def points_callback(self, points: PointCloud2) -> None:
    """Log a `PointCloud2` with `log_points`."""
    time = Time.from_msg(points.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)

    pts = point_cloud2.read_points(points, field_names=["x", "y", "z"],
                                   skip_nans=True)

    # The realsense driver exposes a float field called 'rgb', but the data is
    # actually stored as bytes within the payload (not a float at all). Patch
    # points.field to use the correct r,g,b, offsets so we can extract them with
    # read_points.
    points.fields = [
        PointField(name="r", offset=16, datatype=PointField.UINT8, count=1),
        PointField(name="g", offset=17, datatype=PointField.UINT8, count=1),
        PointField(name="b", offset=18, datatype=PointField.UINT8, count=1),
    ]

    colors = point_cloud2.read_points(points, field_names=["r", "g", "b"],
                                      skip_nans=True)

    pts = structured_to_unstructured(pts)
    colors = colors = structured_to_unstructured(colors)

    # Log points once rigidly under robot/camera/points. This is a robot-centric
    # view of the world.
    rr.log_points("map/robot/camera/points", positions=pts, colors=colors)
    self.log_tf_as_rigid3("map/robot/camera/points", time)
```

### LaserScan to `log_line_segments`
Rerun does not yet have native support for a `LaserScan` style primitive so we need
to do a bit of additional transformation logic (see: [#1534](https://github.com/rerun-io/rerun/issues/1534).)

First, we convert the scan into a point-cloud using the `laser_geometry` package.
After converting to a point-cloud, we extract the pts just as above with `PointCloud2`.

We could have logged the Points directly using `rr.log_points`, but for
the sake of this demo, we wanted to instead log a laser scan as a bunch of lines
in a similar fashion to how it is depicted in gazebo.

We generate a second matching set of points for each ray projected out 0.3m from
the origin and then interlace the two sets of points using numpy hstack and reshape.
This results in a set of alternating points defining rays from the origin to each
laser scan result, which is the format expected by `rr.log_line_segments`:
```python
def __init__(self) -> None:
    # ...
    self.laser_proj = laser_geometry.laser_geometry.LaserProjection()

def scan_callback(self, scan: LaserScan) -> None:
    """Log a LaserScan after transforming it to line-segments."""
    time = Time.from_msg(scan.header.stamp)
    rr.set_time_nanos("ros_time", time.nanoseconds)

    # Project the laser scan to a collection of points
    points = self.laser_proj.projectLaser(scan)
    pts = point_cloud2.read_points(points, field_names=["x", "y", "z"], skip_nans=True)
    pts = structured_to_unstructured(pts)

    # Turn every pt into a line-segment from the origin to the point.
    origin = (pts / np.linalg.norm(pts, axis=1).reshape(-1, 1)) * 0.3
    segs = np.hstack([origin, pts]).reshape(pts.shape[0] * 2, 3)

    rr.log_line_segments("map/robot/scan", segs, stroke_width=0.005)
    self.log_tf_as_rigid3("map/robot/scan", time)
```

### URDF to `log_mesh`
The URDF conversion is actually the most complex operation in this example. As such the functionality
is split out into a separate [rerun/examples/python/ros/rerun_urdf.py](https://github.com/rerun-io/rerun/blob/main/examples/python/ros/rerun_urdf.py)
helper.

Loading the URDF from the `/robot_description` topic is relatively straightforward since we use
[yourdfpy](https://github.com/clemense/yourdfpy) library to do the heavy lifting.
The main complication is that the actual mesh resources in that URDF need to be located via `ament`.
Fortunately, `yourdfpy` accepts a filename handler, which we shim together with
ament `get_package_share_directory`.
```python
def ament_locate_package(fname: str) -> str:
    """Helper to locate urdf resources via ament."""
    if not fname.startswith("package://"):
        return fname
    parsed = urlparse(fname)
    return os.path.join(get_package_share_directory(parsed.netloc),
                        parsed.path[1:])


def load_urdf_from_msg(msg: String) -> URDF:
    """Load a URDF file using `yourdfpy` and find resources via ament."""
    f = io.StringIO(msg.data)
    return URDF.load(f, filename_handler=ament_locate_package)
```

We then use `rerun_urdf.load_urdf_from_msg` from the URDF subscription callback.

Note that when developing this guide, we noticed that the camera mesh URDF was not having
its scale applied to it. This seems like a bug in either `yourdfpy` or `pycollada`
not respecting the scale hint.  To accommodate this, we manually re-scale the
camera link.

Once we have correctly re-scaled the camera component, we can send the whole scene to rerun with
`rerun_urdf.log_scene`. 
```python
def urdf_callback(self, urdf_msg: String) -> None:
    """Log a URDF using `log_scene` from `rerun_urdf`."""
    urdf = load_urdf_from_msg(urdf_msg)

    # The turtlebot URDF appears to have scale set incorrectly for the
    # camera_link. Although rviz loads it properly `yourdfpy` does not.
    orig, _ = urdf.scene.graph.get("camera_link")
    scale = trimesh.transformations.scale_matrix(0.00254)
    urdf.scene.graph.update(frame_to="camera_link", matrix=orig.dot(scale))
    scaled = urdf.scene.scaled(1.0)

    rerun_urdf.log_scene(scene=scaled,
                         node=urdf.base_link,
                         path="map/robot/urdf",
                         timeless=True)
```

Back in `rerun_urdf.log_scene` all the code is doing is recursively walking through
the trimesh scene graph. For each node, it extracts the transform to the parent,
which it logs via `rr.log_rigid3` before then using `rr.log_mesh` to send the vertices,
indices, and normals from the trimesh geometry. This code is almost entirely
URDF-independent and is a good candidate for a future Python API ([#1536](https://github.com/rerun-io/rerun/issues/1536).)
```python
node_data = scene.graph.get(frame_to=node, frame_from=parent)

if node_data:
    # Log the transform between this node and its direct parent (if it has one!).
    if parent:
        world_from_mesh = node_data[0]
        t = trimesh.transformations.translation_from_matrix(world_from_mesh)
        q = trimesh.transformations.quaternion_from_matrix(world_from_mesh)
        # `trimesh` stores quaternions in `wxyz` format, rerun needs `xyzw`
        # TODO(jleibs): Remove conversion once
        # [#883](https://github.com/rerun-io/rerun/issues/883) is closed
        q = np.array([q[1], q[2], q[3], q[0]])
        rr.log_rigid3(path, parent_from_child=(t, q), timeless=timeless)

    # Log this node's mesh, if it has one.
    mesh = cast(trimesh.Trimesh, scene.geometry.get(node_data[1]))
    if mesh:
        # ... extract some color information
        rr.log_mesh(
            path,
            mesh.vertices,
            indices=mesh.faces,
            normals=mesh.vertex_normals,
            albedo_factor=albedo_factor,
            timeless=timeless,
        )
```
Color data is also extracted from the trimesh, but omitted here for brevity.

## In Summary
Although there is a non-trivial amount of code, none of it is overly complicated. Each message callback
operates independently of the others, processing an incoming message, adapting it to Rerun and then
logging it again.

There are several places where Rerun is currently missing support for primitives that will further
simplify this implementation. We will continue to update this guide as new functionality becomes
available.

While this guide has only covered a small fraction of the possible ROS messages that could
be sent to Rerun, hopefully, it has given you some tools to apply to your project.

If you find that specific functionality is lacking for your use case, please provide more
context in the existing issues or [open an new one](https://github.com/rerun-io/rerun/issues/new/choose) on Github.
