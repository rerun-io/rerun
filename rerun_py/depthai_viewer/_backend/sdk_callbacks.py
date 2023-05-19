from typing import Callable, List, Optional, Tuple

import cv2
import depthai as dai
import numpy as np
from ahrs.filters import Mahony
from depthai_sdk.classes.packets import (
    DepthPacket,
    DetectionPacket,
    FramePacket,
    IMUPacket,
    # PointcloudPacket,
    TwoStagePacket,
    _Detection,
)
from numpy.typing import NDArray

import depthai_viewer as viewer
from depthai_viewer._backend import classification_labels
from depthai_viewer._backend.store import Store
from depthai_viewer._backend.topic import Topic
from depthai_viewer.components.rect2d import RectFormat


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
    _get_camera_intrinsics: Callable[[dai.CameraBoardSocket, int, int], NDArray[np.float32]]

    def __init__(self, store: Store):
        viewer.init("Depthai Viewer")
        viewer.connect()
        self.store = store
        self.ahrs = Mahony(frequency=100)
        self.ahrs.Q = np.array([1, 0, 0, 0], dtype=np.float64)

    def set_camera_intrinsics_getter(
        self, camera_intrinsics_getter: Callable[[dai.CameraBoardSocket, int, int], NDArray[np.float32]]
    ) -> None:
        self._get_camera_intrinsics = camera_intrinsics_getter

    def on_imu(self, packet: IMUPacket) -> None:
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
        viewer.log_imu([accel.z, accel.x, accel.y], [gyro.z, gyro.x, gyro.y], self.ahrs.Q, [mag.x, mag.y, mag.z])

    def on_color_frame(self, frame: FramePacket) -> None:
        # Always log pinhole cam and pose (TODO(filip): move somewhere else or not)
        if Topic.ColorImage not in self.store.subscriptions:
            return
        viewer.log_rigid3(EntityPath.RGB_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        h, w, _ = frame.frame.shape
        viewer.log_pinhole(
            EntityPath.RGB_PINHOLE_CAMERA,
            child_from_parent=self._get_camera_intrinsics(dai.CameraBoardSocket.RGB, w, h),
            width=w,
            height=h,
        )
        viewer.log_image(EntityPath.RGB_CAMERA_IMAGE, cv2.cvtColor(frame.frame, cv2.COLOR_BGR2RGB))

    def on_left_frame(self, frame: FramePacket) -> None:
        if Topic.LeftMono not in self.store.subscriptions:
            return
        h, w = frame.frame.shape
        viewer.log_rigid3(EntityPath.MONO_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        viewer.log_pinhole(
            EntityPath.LEFT_PINHOLE_CAMERA,
            child_from_parent=self._get_camera_intrinsics(dai.CameraBoardSocket.LEFT, w, h),
            width=w,
            height=h,
        )
        viewer.log_image(EntityPath.LEFT_CAMERA_IMAGE, frame.frame)

    def on_right_frame(self, frame: FramePacket) -> None:
        if Topic.RightMono not in self.store.subscriptions:
            return
        h, w = frame.frame.shape
        viewer.log_rigid3(EntityPath.MONO_CAMERA_TRANSFORM, child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        viewer.log_pinhole(
            EntityPath.RIGHT_PINHOLE_CAMERA,
            child_from_parent=self._get_camera_intrinsics(dai.CameraBoardSocket.RIGHT, w, h),
            width=w,
            height=h,
        )
        viewer.log_image(EntityPath.RIGHT_CAMERA_IMAGE, frame.frame)

    def on_stereo_frame(self, frame: DepthPacket) -> None:
        if Topic.DepthImage not in self.store.subscriptions:
            return
        depth_frame = frame.frame
        path = EntityPath.RGB_PINHOLE_CAMERA + "/Depth"
        if not self.store.pipeline_config or not self.store.pipeline_config.depth:
            # Essentially impossible to get here
            return
        depth = self.store.pipeline_config.depth
        if depth.align == dai.CameraBoardSocket.LEFT:
            path = EntityPath.LEFT_PINHOLE_CAMERA + "/Depth"
        elif depth.align == dai.CameraBoardSocket.RIGHT:
            path = EntityPath.RIGHT_PINHOLE_CAMERA + "/Depth"
        viewer.log_depth_image(path, depth_frame, meter=1e3)

    def on_detections(self, packet: DetectionPacket) -> None:
        rects, colors, labels = self._detections_to_rects_colors_labels(packet)
        viewer.log_rects(EntityPath.DETECTIONS, rects, rect_format=RectFormat.XYXY, colors=colors, labels=labels)

    def _detections_to_rects_colors_labels(
        self, packet: DetectionPacket, omz_labels: Optional[List[str]] = None
    ) -> Tuple[List[List[int]], List[List[int]], List[str]]:
        rects = []
        colors = []
        labels = []
        for detection in packet.detections:
            rects.append(self._rect_from_detection(detection))
            colors.append([0, 255, 0])
            label: str = detection.label
            # Open model zoo models output label index
            if omz_labels is not None and isinstance(label, int):
                label += omz_labels[label]
            label += ", " + str(int(detection.img_detection.confidence * 100)) + "%"
            labels.append(label)
        return rects, colors, labels

    def on_yolo_packet(self, packet: DetectionPacket) -> None:
        rects, colors, labels = self._detections_to_rects_colors_labels(packet)
        viewer.log_rects(EntityPath.DETECTIONS, rects=rects, colors=colors, labels=labels, rect_format=RectFormat.XYXY)

    def on_age_gender_packet(self, packet: TwoStagePacket) -> None:
        for det, rec in zip(packet.detections, packet.nnData):
            age = int(float(np.squeeze(np.array(rec.getLayerFp16("age_conv3")))) * 100)
            gender = np.squeeze(np.array(rec.getLayerFp16("prob")))
            gender_str = "Woman" if gender[0] > gender[1] else "Man"
            label = f"{gender_str}, {age}"
            color = [255, 0, 0] if gender[0] > gender[1] else [0, 0, 255]
            # TODO(filip): maybe use viewer.log_annotation_context to log class colors for detections
            viewer.log_rect(
                EntityPath.DETECTION,
                self._rect_from_detection(det),
                rect_format=RectFormat.XYXY,
                color=color,
                label=label,
            )

    def _rect_from_detection(self, detection: _Detection) -> List[int]:
        return [
            *detection.bottom_right,
            *detection.top_left,
        ]

    def on_mobilenet_ssd_packet(self, packet: DetectionPacket) -> None:
        rects, colors, labels = self._detections_to_rects_colors_labels(packet, classification_labels.MOBILENET_LABELS)
        viewer.log_rects(EntityPath.DETECTIONS, rects=rects, colors=colors, labels=labels, rect_format=RectFormat.XYXY)
