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
        self.model = PinholeCameraModel()
        self.cv_bridge = cv_bridge.CvBridge()
        self.laser_proj = laser_geometry.laser_geometry.LaserProjection()
        self.subscribers: list[rclpy.Subscription] = []

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
        time = Time.from_msg(info.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))

        self.model.from_camera_info(info)

        # TODO: remove `from_fields` when Pinhole constructor patch is released: https://github.com/rerun-io/rerun/pull/12360
        rr.log(
            "map/robot/camera/img",
            rr.Pinhole.from_fields(
                resolution=[self.model.width, self.model.height],
                image_from_camera=self.model.intrinsic_matrix(),
                parent_frame=info.header.frame_id,
                child_frame=info.header.frame_id + "_image_plane",
            ),
        )

    def odom_callback(self, odom: Odometry) -> None:
        time = Time.from_msg(odom.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))

        # Capture time-series data for the linear and angular velocities
        rr.log("odometry/vel", rr.Scalars(odom.twist.twist.linear.x))
        rr.log("odometry/ang_vel", rr.Scalars(odom.twist.twist.angular.z))

    def image_callback(self, img: Image) -> None:
        time = Time.from_msg(img.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))

        rr.log("map/robot/camera/img", rr.Image(self.cv_bridge.imgmsg_to_cv2(img)))
        rr.log("map/robot/camera/img", rr.CoordinateFrame(frame=img.header.frame_id + "_image_plane"))

    def depth_callback(self, img: Image) -> None:
        time = Time.from_msg(img.header.stamp)
        rr.set_time("ros_time", timestamp=np.datetime64(time.nanoseconds, "ns"))

        rr.log(
            "map/robot/camera/img/depth",
            rr.DepthImage(self.cv_bridge.imgmsg_to_cv2(img, desired_encoding="32FC1"), meter=1.0, colormap="viridis"),
        )
        rr.log("map/robot/camera/img/depth", rr.CoordinateFrame(frame=img.header.frame_id + "_image_plane"))

    def scan_callback(self, scan: LaserScan) -> None:
        """
        Log a LaserScan after transforming it to line-segments.

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

        rr.log("map/robot/scan", rr.LineStrips3D(segs, radii=0.0025))
        rr.log("map/robot/scan", rr.CoordinateFrame(frame=scan.header.frame_id))

    def urdf_callback(self, urdf_msg: String) -> None:
        # TODO: file_path is not known here, robot.urdf is just a placeholder to let Rerun know the file type.
        rr.log_file_from_contents(
            file_path="robot.urdf",
            file_contents=urdf_msg.data.encode("utf-8"),
            entity_path_prefix="map/robot/urdf",
            static=True,
        )

    def tf_callback(self, tf_msg: TFMessage) -> None:
        time = None
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
