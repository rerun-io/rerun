#!/usr/bin/env python3
"""
Simple example of a ROS node that republishes some common types to Rerun.

The solution here is mostly a toy example to show how ROS concepts can be
mapped to Rerun. For more information on future improved ROS support,
see the tracking issue: <https://github.com/rerun-io/rerun/issues/1537>.

NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.
"""

from __future__ import annotations

import argparse
import sys
from typing import Callable

import numpy as np
import rerun as rr  # pip install rerun-sdk

try:
    import cv_bridge
    import laser_geometry
    import rclpy
    from image_geometry import PinholeCameraModel
    from nav_msgs.msg import Odometry
    from numpy.lib.recfunctions import structured_to_unstructured
    from rclpy.callback_groups import ReentrantCallbackGroup
    from rclpy.node import Node
    from rclpy.qos import QoSDurabilityPolicy, QoSProfile
    from rclpy.time import Time
    from sensor_msgs.msg import CameraInfo, Image, LaserScan
    from sensor_msgs_py import point_cloud2
    from std_msgs.msg import String
    from tf2_msgs.msg import TFMessage

except ImportError:
    print(
        """
Could not import the required ROS2 packages.

Make sure you have installed ROS2 (https://docs.ros.org/en/kilted/index.html)
and sourced /opt/ros/kilted/setup.bash

See: README.md for more details.
""",
    )
    sys.exit(1)


class TurtleSubscriber(Node):  # type: ignore[misc]
    def __init__(self) -> None:
        super().__init__("rr_turtlebot")

        # Assorted helpers for data conversions
        self.pinhole_model = PinholeCameraModel()
        self.cv_bridge = cv_bridge.CvBridge()
        self.laser_proj = laser_geometry.laser_geometry.LaserProjection()
        self.subscribers: list[rclpy.Subscription] = []

        # Subscribe to the topics we want to republish to Rerun.
        # See the callback methods below for how each message type is handled.
        self.subscribe("/tf", TFMessage, self.tf_callback)
        self.subscribe("/tf_static", TFMessage, self.tf_callback, latching=True)
        self.subscribe("/odom", Odometry, self.odom_callback)
        self.subscribe("/scan", LaserScan, self.scan_callback)
        self.subscribe("/rgbd_camera/camera_info", CameraInfo, self.cam_info_callback)
        self.subscribe("/rgbd_camera/image", Image, self.image_callback)
        self.subscribe("/rgbd_camera/depth_image", Image, self.depth_callback)
        self.subscribe("/robot_description", String, self.urdf_callback, latching=True)

    def subscribe(
        self, topic: str, msg_type: type, callback: Callable[[rclpy.MsgT], None], latching: bool = False
    ) -> None:
        """Adds a subscriber to a topic with the given message type and callback."""
        # `qos_profile` can either be an int (history depth) or a QoSProfile.
        # See: https://docs.ros.org/en/rolling/p/rclpy/rclpy.node.html#rclpy.node.Node.create_subscription
        qos_profile = QoSProfile(depth=1, durability=QoSDurabilityPolicy.TRANSIENT_LOCAL) if latching else 10
        sub = self.create_subscription(
            msg_type=msg_type,
            topic=topic,
            callback=callback,
            qos_profile=qos_profile,
            callback_group=ReentrantCallbackGroup(),  # allow concurrent callbacks
        )
        self.subscribers.append(sub)

    def cam_info_callback(self, info: CameraInfo) -> None:
        """
        Logs CameraInfo as a Rerun Pinhole.
        """
        time = Time.from_msg(info.header.stamp)
        self.pinhole_model.from_camera_info(info)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))
        # TODO(michael): remove `from_fields` when Pinhole constructor patch is released: https://github.com/rerun-io/rerun/pull/12360
        rr.log(
            "rgbd_camera/camera_info",
            rr.Pinhole.from_fields(
                resolution=[info.width, info.height],
                image_from_camera=self.pinhole_model.intrinsic_matrix(),
                image_plane_distance=1.0,
                parent_frame=info.header.frame_id,
                # Specifying a `child_frame` for the 2D image plane allows Rerun to
                # visualize the pinhole frustum together with the image in 3D views.
                # This has to match the coordinate frames used when logging images,
                # see `image_callback` below.
                child_frame=info.header.frame_id + "_image_plane",
            ),
        )

    def odom_callback(self, odom: Odometry) -> None:
        """
        Logs data from Odometry as Rerun Scalars.
        """
        time = Time.from_msg(odom.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))
        # Capture time-series data for the linear and angular velocities
        rr.log("odom/twist/linear/x", rr.Scalars(odom.twist.twist.linear.x))
        rr.log("odom/twist/angular/z", rr.Scalars(odom.twist.twist.angular.z))

    def image_callback(self, img: Image) -> None:
        """
        Logs an RGB image as a Rerun Image.
        """
        time = Time.from_msg(img.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))
        rr.log("rgbd_camera/image", rr.Image(self.cv_bridge.imgmsg_to_cv2(img)))
        # Make sure the image plane frame matches what we set in `cam_info_callback`.
        rr.log("rgbd_camera/image", rr.CoordinateFrame(frame=img.header.frame_id + "_image_plane"))

    def depth_callback(self, img: Image) -> None:
        """
        Logs a depth image as a Rerun DepthImage.
        """
        time = Time.from_msg(img.header.stamp)
        depth_image = rr.DepthImage(
            self.cv_bridge.imgmsg_to_cv2(img, desired_encoding="32FC1"),
            meter=1.0,
            colormap="viridis",
        )
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))
        rr.log("rgbd_camera/depth_image", depth_image)
        rr.log("rgbd_camera/depth_image", rr.CoordinateFrame(frame=img.header.frame_id + "_image_plane"))

    def scan_callback(self, scan: LaserScan) -> None:
        """
        Logs a LaserScan after transforming it to line-segments.

        Note: we do a client-side transformation of the LaserScan data into Rerun
        points / lines until Rerun has native support for LaserScan style projections:
        [#1534](https://github.com/rerun-io/rerun/issues/1534)
        """
        time = Time.from_msg(scan.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))

        # Project the laser scan to a collection of points
        points = self.laser_proj.projectLaser(scan)
        pts = point_cloud2.read_points(points, field_names=["x", "y", "z"], skip_nans=True)
        pts = structured_to_unstructured(pts)

        # Turn every pt into a line-segment from the origin to the point.
        origin = (pts / np.linalg.norm(pts, axis=1).reshape(-1, 1)) * 0.3
        segs = np.hstack([origin, pts]).reshape(pts.shape[0] * 2, 3)

        rr.log("scan", rr.LineStrips3D(segs, radii=0.0025, colors=[255, 165, 0]))
        rr.log("scan", rr.CoordinateFrame(frame=scan.header.frame_id))

    def urdf_callback(self, urdf_msg: String) -> None:
        """
        Forwards the robot description message to Rerun's built-in URDF loader.

        Documentation about URDF support in Rerun can be found here:
        https://rerun.io/docs/howto/urdf
        """
        # NOTE: file_path is not known here, robot.urdf is just a placeholder to let
        # Rerun know the file type. Since we run this example in a ROS environment,
        # Rerun can use AMENT_PREFIX_PATH etc to resolve asset paths of the URDF.
        rr.log_file_from_contents(
            file_path="robot.urdf",
            file_contents=urdf_msg.data.encode("utf-8"),
            entity_path_prefix="urdf",
            static=True,
        )

    def tf_callback(self, tf_msg: TFMessage) -> None:
        """
        Logs TF transforms to Rerun as Transform3D messages,
        with `parent_frame` and `child_frame` fields set.

        Documentation about transforms in Rerun can be found here:
        https://rerun.io/docs/concepts/transforms
        """
        for transform in tf_msg.transforms:
            time = Time.from_msg(transform.header.stamp)
            rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))
            rr.log(
                "transforms",
                rr.Transform3D(
                    translation=[
                        transform.transform.translation.x,
                        transform.transform.translation.y,
                        transform.transform.translation.z,
                    ],
                    rotation=rr.Quaternion(
                        xyzw=[
                            transform.transform.rotation.x,
                            transform.transform.rotation.y,
                            transform.transform.rotation.z,
                            transform.transform.rotation.w,
                        ]
                    ),
                    parent_frame=transform.header.frame_id,
                    child_frame=transform.child_frame_id,
                ),
            )


def main() -> None:
    parser = argparse.ArgumentParser(description="Simple example of a ROS node that republishes to Rerun.")
    rr.script_add_args(parser)
    args, unknownargs = parser.parse_known_args()
    rr.script_setup(args, "rerun_example_ros_node")

    # Any remaining args go to rclpy
    rclpy.init(args=unknownargs)

    turtle_subscriber = TurtleSubscriber()

    # Use the MultiThreadedExecutor so that calls to `lookup_transform` don't block the other threads
    rclpy.spin(turtle_subscriber, executor=rclpy.executors.MultiThreadedExecutor())

    turtle_subscriber.destroy_node()
    rclpy.shutdown()


if __name__ == "__main__":
    main()
