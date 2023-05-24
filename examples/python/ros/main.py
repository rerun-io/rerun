#!/usr/bin/env python3
"""
Simple example of a ROS node that republishes some common types to Rerun.

The solution here is mostly a toy example to show how ROS concepts can be
mapped to Rerun. Fore more information on future improved ROS support,
see the tracking issue: https://github.com/rerun-io/rerun/issues/1537

NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.
"""

import argparse
import sys

import numpy as np
import rerun as rr  # pip install rerun-sdk

try:
    import cv_bridge
    import laser_geometry
    import rclpy
    import rerun_urdf
    import trimesh
    from image_geometry import PinholeCameraModel
    from nav_msgs.msg import Odometry
    from numpy.lib.recfunctions import structured_to_unstructured
    from rclpy.callback_groups import ReentrantCallbackGroup
    from rclpy.node import Node
    from rclpy.qos import QoSDurabilityPolicy, QoSProfile
    from rclpy.time import Duration, Time
    from sensor_msgs.msg import CameraInfo, Image, LaserScan, PointCloud2, PointField
    from sensor_msgs_py import point_cloud2
    from std_msgs.msg import String
    from tf2_ros import TransformException
    from tf2_ros.buffer import Buffer
    from tf2_ros.transform_listener import TransformListener

except ImportError:
    print(
        """
Could not import the required ROS2 packages.

Make sure you have installed ROS2 (https://docs.ros.org/en/humble/index.html)
and sourced /opt/ros/humble/setup.bash

See: README.md for more details.
"""
    )
    sys.exit(1)


class TurtleSubscriber(Node):  # type: ignore[misc]
    def __init__(self) -> None:
        super().__init__("rr_turtlebot")

        # Used for subscribing to latching topics
        latching_qos = QoSProfile(depth=1, durability=QoSDurabilityPolicy.TRANSIENT_LOCAL)

        # Allow concurrent callbacks
        self.callback_group = ReentrantCallbackGroup()

        # Subscribe to TF topics
        self.tf_buffer = Buffer()
        self.tf_listener = TransformListener(self.tf_buffer, self)

        # Define a mapping for transforms
        self.path_to_frame = {
            "map": "map",
            "map/points": "camera_depth_frame",
            "map/robot": "base_footprint",
            "map/robot/scan": "base_scan",
            "map/robot/camera": "camera_rgb_optical_frame",
            "map/robot/camera/points": "camera_depth_frame",
        }

        # Assorted helpers for data conversions
        self.model = PinholeCameraModel()
        self.cv_bridge = cv_bridge.CvBridge()
        self.laser_proj = laser_geometry.laser_geometry.LaserProjection()

        # Log a bounding box as a visual placeholder for the map
        # # TODO(jleibs): Log the real map once [#1531](https://github.com/rerun-io/rerun/issues/1531) is merged
        rr.log_obb(
            "map/box",
            half_size=[3, 3, 1],
            position=[0, 0, 1],
            color=[255, 255, 255, 255],
            timeless=True,
        )

        # Subscriptions
        self.info_sub = self.create_subscription(
            CameraInfo,
            "/intel_realsense_r200_depth/camera_info",
            self.cam_info_callback,
            10,
            callback_group=self.callback_group,
        )

        self.odom_sub = self.create_subscription(
            Odometry,
            "/odom",
            self.odom_callback,
            10,
            callback_group=self.callback_group,
        )

        self.img_sub = self.create_subscription(
            Image,
            "/intel_realsense_r200_depth/image_raw",
            self.image_callback,
            10,
            callback_group=self.callback_group,
        )

        self.points_sub = self.create_subscription(
            PointCloud2,
            "/intel_realsense_r200_depth/points",
            self.points_callback,
            10,
            callback_group=self.callback_group,
        )

        self.scan_sub = self.create_subscription(
            LaserScan,
            "/scan",
            self.scan_callback,
            10,
            callback_group=self.callback_group,
        )

        # The urdf is published as latching
        self.urdf_sub = self.create_subscription(
            String,
            "/robot_description",
            self.urdf_callback,
            qos_profile=latching_qos,
            callback_group=self.callback_group,
        )

    def log_tf_as_rigid3(self, path: str, time: Time) -> None:
        """
        Helper to look up a transform with tf and log using `log_transform3d`.

        Note: we do the lookup on the client side instead of re-logging the raw transforms until
        Rerun has support for Derived Transforms [#1533](https://github.com/rerun-io/rerun/issues/1533)
        """
        # Get the parent path
        parent_path = path.rsplit("/", 1)[0]

        # Find the corresponding frames from the mapping
        child_frame = self.path_to_frame[path]
        parent_frame = self.path_to_frame[parent_path]

        # Do the TF lookup to get transform from child (source) -> parent (target)
        try:
            tf = self.tf_buffer.lookup_transform(parent_frame, child_frame, time, timeout=Duration(seconds=0.1))
            t = tf.transform.translation
            q = tf.transform.rotation
            rr.log_transform3d(
                path, rr.TranslationRotationScale3D([t.x, t.y, t.z], rr.Quaternion([q.x, q.y, q.z, q.w]))
            )
        except TransformException as ex:
            print("Failed to get transform: {}".format(ex))

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

    def odom_callback(self, odom: Odometry) -> None:
        """Update transforms when odom is updated."""
        time = Time.from_msg(odom.header.stamp)
        rr.set_time_nanos("ros_time", time.nanoseconds)

        # Capture time-series data for the linear and angular velocities
        rr.log_scalar("odometry/vel", odom.twist.twist.linear.x)
        rr.log_scalar("odometry/ang_vel", odom.twist.twist.angular.z)

        # Update the robot pose itself via TF
        self.log_tf_as_rigid3("map/robot", time)

    def image_callback(self, img: Image) -> None:
        """Log an `Image` with `log_image` using `cv_bridge`."""
        time = Time.from_msg(img.header.stamp)
        rr.set_time_nanos("ros_time", time.nanoseconds)

        rr.log_image("map/robot/camera/img", self.cv_bridge.imgmsg_to_cv2(img))
        self.log_tf_as_rigid3("map/robot/camera", time)

    def points_callback(self, points: PointCloud2) -> None:
        """Log a `PointCloud2` with `log_points`."""
        time = Time.from_msg(points.header.stamp)
        rr.set_time_nanos("ros_time", time.nanoseconds)

        pts = point_cloud2.read_points(points, field_names=["x", "y", "z"], skip_nans=True)

        # The realsense driver exposes a float field called 'rgb', but the data is actually stored
        # as bytes within the payload (not a float at all). Patch points.field to use the correct
        # r,g,b, offsets so we can extract them with read_points.
        points.fields = [
            PointField(name="r", offset=16, datatype=PointField.UINT8, count=1),
            PointField(name="g", offset=17, datatype=PointField.UINT8, count=1),
            PointField(name="b", offset=18, datatype=PointField.UINT8, count=1),
        ]

        colors = point_cloud2.read_points(points, field_names=["r", "g", "b"], skip_nans=True)

        pts = structured_to_unstructured(pts)
        colors = colors = structured_to_unstructured(colors)

        # Log points once rigidly under robot/camera/points. This is a robot-centric
        # view of the world.
        rr.log_points("map/robot/camera/points", positions=pts, colors=colors)
        self.log_tf_as_rigid3("map/robot/camera/points", time)

        # Log points a second time after transforming to the map frame. This is a map-centric
        # view of the world.
        #
        # Once Rerun supports fixed-frame aware transforms [#1522](https://github.com/rerun-io/rerun/issues/1522)
        # this will no longer be necessary.
        rr.log_points("map/points", positions=pts, colors=colors)
        self.log_tf_as_rigid3("map/points", time)

    def scan_callback(self, scan: LaserScan) -> None:
        """
        Log a LaserScan after transforming it to line-segments.

        Note: we do a client-side transformation of the LaserScan data into Rerun
        points / lines until Rerun has native support for LaserScan style projections:
        [#1534](https://github.com/rerun-io/rerun/issues/1534)
        """
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

    def urdf_callback(self, urdf_msg: String) -> None:
        """Log a URDF using `log_scene` from `rerun_urdf`."""
        urdf = rerun_urdf.load_urdf_from_msg(urdf_msg)

        # The turtlebot URDF appears to have scale set incorrectly for the camera-link
        # Although rviz loads it properly `yourdfpy` does not.
        orig, _ = urdf.scene.graph.get("camera_link")
        scale = trimesh.transformations.scale_matrix(0.00254)
        urdf.scene.graph.update(frame_to="camera_link", matrix=orig.dot(scale))
        scaled = urdf.scene.scaled(1.0)

        rerun_urdf.log_scene(scene=scaled, node=urdf.base_link, path="map/robot/urdf", timeless=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Simple example of a ROS node that republishes to Rerun.")
    rr.script_add_args(parser)
    args, unknownargs = parser.parse_known_args()
    rr.script_setup(args, "turtlebot_viz")

    # Any remaining args go to rclpy
    rclpy.init(args=unknownargs)

    turtle_subscriber = TurtleSubscriber()

    # Use the MultiThreadedExecutor so that calls to `lookup_transform` don't block the other threads
    rclpy.spin(turtle_subscriber, executor=rclpy.executors.MultiThreadedExecutor())

    turtle_subscriber.destroy_node()
    rclpy.shutdown()


if __name__ == "__main__":
    main()
