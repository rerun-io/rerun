from typing import Callable, Dict, List, Tuple, Union

import cv2
import depthai as dai
import numpy as np
import rerun as rr
from ahrs.filters import Mahony
from depthai_sdk.classes.packets import (
    DepthPacket,
    DetectionPacket,
    FramePacket,
    IMUPacket,
    # PointcloudPacket,
    TwoStagePacket,
)
from rerun.components.rect2d import RectFormat

from depthai_viewer_backend import classification_labels
from depthai_viewer_backend.store import Store
from depthai_viewer_backend.topic import Topic


class EntityPath:
    LEFT_PINHOLE_CAMERA = "mono/camera/left_mono"
    LEFT_CAMERA_IMAGE = "mono/camera/left_mono/Left mono"
    RIGHT_PINHOLE_CAMERA = "mono/camera/right_mono"
    RIGHT_CAMERA_IMAGE = "mono/camera/right_mono/Right mono"
    RGB_PINHOLE_CAMERA = "color/camera/rgb"
    RGB_CAMERA_IMAGE = "color/camera/rgb/Color camera"

    DETECTIONS = "color/camera/rgb/Detections"
    DETECTION = "color/camera/rgb/Detection"

    RGB_CAMERA_TRANSFORM = "color/camera"
    MONO_CAMERA_TRANSFORM = "mono/camera"


class SdkCallbacks:
    store: Store
    ahrs: Mahony
    _get_camera_intrinsics: Callable[[int, int], np.ndarray]

    def __init__(self, store: Store):
        rr.init("Depthai Viewer")
        rr.connect()
        self.store = store
        self.ahrs = Mahony(frequency=100)
        self.ahrs.Q = np.array([1, 0, 0, 0], dtype=np.float64)

    def set_camera_intrinsics_getter(self, camera_intrinsics_getter: Callable[[int, int], np.ndarray]):
        self._get_camera_intrinsics = camera_intrinsics_getter

    def on_imu(self, packet: IMUPacket):
        for data in packet.data:
            gyro: dai.IMUReportGyroscope = data.gyroscope
            accel: dai.IMUReportAccelerometer = data.acceleroMeter
            mag: dai.IMUReportMagneticField = data.magneticField
            # TODO(filip): Move coordinate mapping to sdk
            self.ahrs.Q = self.ahrs.updateIMU(
                self.ahrs.Q, np.array([gyro.z, gyro.x, gyro.y]), np.array([accel.z, accel.x, accel.y])
            )
        if Topic.ImuData not in self.store.subscriptions:
            return
        rr.log_imu([accel.z, accel.x, accel.y], [gyro.z, gyro.x, gyro.y], self.ahrs.Q, [mag.x, mag.y, mag.z])

    # def on_pointcloud(self, packet: PointcloudPacket):
    #     # if Topic.PointCloud not in self.store.subscriptions:
    #     #     return
    #     colors = cv2.cvtColor(packet.color_frame.getCvFrame(), cv2.COLOR_BGR2RGB).reshape(-1, 3)
    #     points = packet.points.reshape(-1, 3)

    #     path = EntityPath.RGB_CAMERA_TRANSFORM + "/Point cloud"
    #     depth = self.store.pipeline_config.depth
    #     if not depth:
    #         # Essentially impossible to get here
    #         return
    #     if depth.align == dai.CameraBoardSocket.LEFT or depth.align == dai.CameraBoardSocket.RIGHT:
    #         path = EntityPath.MONO_CAMERA_TRANSFORM + "/Point cloud"
    #     rr.log_points(path, points, colors=colors)

    def on_color_frame(self, frame: FramePacket):
        # Always log pinhole cam and pose (TODO(filip): move somewhere else or not)
        if Topic.ColorImage not in self.store.subscriptions:
            return
        rr.log_rigid3(EntityPath.RGB_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        w, h = frame.msg.getWidth(), frame.msg.getHeight()
        rr.log_pinhole(
            EntityPath.RGB_PINHOLE_CAMERA, child_from_parent=self._get_camera_intrinsics(w, h), width=w, height=h
        )
        rr.log_image(EntityPath.RGB_CAMERA_IMAGE, cv2.cvtColor(frame.frame, cv2.COLOR_BGR2RGB))

    def on_left_frame(self, frame: FramePacket):
        if Topic.LeftMono not in self.store.subscriptions:
            return
        w, h = frame.msg.getWidth(), frame.msg.getHeight()
        rr.log_rigid3(EntityPath.MONO_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        rr.log_pinhole(
            EntityPath.LEFT_PINHOLE_CAMERA, child_from_parent=self._get_camera_intrinsics(w, h), width=w, height=h
        )
        rr.log_image(EntityPath.LEFT_CAMERA_IMAGE, frame.frame)

    def on_right_frame(self, frame: FramePacket):
        if Topic.RightMono not in self.store.subscriptions:
            return
        w, h = frame.msg.getWidth(), frame.msg.getHeight()
        rr.log_rigid3(EntityPath.MONO_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        rr.log_pinhole(
            EntityPath.RIGHT_PINHOLE_CAMERA, child_from_parent=self._get_camera_intrinsics(w, h), width=w, height=h
        )
        rr.log_image(EntityPath.RIGHT_CAMERA_IMAGE, frame.frame)

    def on_stereo_frame(self, frame: DepthPacket):
        if Topic.DepthImage not in self.store.subscriptions:
            return
        depth_frame = frame.frame
        path = EntityPath.RGB_PINHOLE_CAMERA + "/Depth"
        depth = self.store.pipeline_config.depth
        if not depth:
            # Essentially impossible to get here
            return
        if depth.align == dai.CameraBoardSocket.LEFT:
            path = EntityPath.LEFT_PINHOLE_CAMERA + "/Depth"
        elif depth.align == dai.CameraBoardSocket.RIGHT:
            path = EntityPath.RIGHT_PINHOLE_CAMERA + "/Depth"
        rr.log_depth_image(path, depth_frame, meter=1e3)

    def on_detections(self, packet: DetectionPacket):
        rects, colors, labels = self._detections_to_rects_colors_labels(packet)
        rr.log_rects(EntityPath.DETECTIONS, rects, rect_format=RectFormat.XYXY, colors=colors, labels=labels)

    def _detections_to_rects_colors_labels(
        self, packet: DetectionPacket, labels_dict: Union[Dict, None] = None
    ) -> Tuple[List, List, List]:
        h, w, _ = packet.frame.shape
        rects = []
        colors = []
        labels = []
        for detection in packet.img_detections.detections:
            rects.append(
                [
                    max(detection.xmin, 0) * w,
                    max(detection.ymin, 0) * h,
                    min(detection.xmax, 1) * w,
                    min(detection.ymax, 1) * h,
                ]
            )
            colors.append([0, 255, 0])
            label = ""
            if labels_dict is not None:
                label += labels_dict[detection.label] + ", "
            label += str(int(detection.confidence * 100)) + "%"
            labels.append(label)
        return rects, colors, labels

    def on_yolo_packet(self, packet: DetectionPacket):
        rects, colors, labels = self._detections_to_rects_colors_labels(packet, classification_labels.YOLO_TINY_LABELS)
        rr.log_rects(EntityPath.DETECTIONS, rects=rects, colors=colors, labels=labels, rect_format=RectFormat.XYXY)

    def on_age_gender_packet(self, packet: TwoStagePacket):
        for det, rec in zip(packet.detections, packet.nnData):
            age = int(float(np.squeeze(np.array(rec.getLayerFp16("age_conv3")))) * 100)
            gender = np.squeeze(np.array(rec.getLayerFp16("prob")))
            gender_str = "Woman" if gender[0] > gender[1] else "Man"
            label = f"{gender_str}, {age}"
            color = [255, 0, 0] if gender[0] > gender[1] else [0, 0, 255]
            x0, y0, x1, y1 = det.get_bbox()
            # TODO(filip): maybe use rr.log_annotation_context to log class colors for detections
            rr.log_rect(
                EntityPath.DETECTION,
                [
                    x0 * packet.frame.shape[1],
                    y0 * packet.frame.shape[0],
                    x1 * packet.frame.shape[1],
                    y1 * packet.frame.shape[0],
                ],
                rect_format=RectFormat.XYXY,
                color=color,
                label=label,
            )

    def on_mobilenet_ssd_packet(self, packet: DetectionPacket):
        rects, colors, labels = self._detections_to_rects_colors_labels(packet, classification_labels.MOBILENET_LABELS)
        rr.log_rects(EntityPath.DETECTIONS, rects=rects, colors=colors, labels=labels, rect_format=RectFormat.XYXY)
