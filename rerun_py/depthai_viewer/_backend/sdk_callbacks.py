from typing import Any, Callable, List, Optional, Tuple, Union

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
from pydantic import BaseModel

import depthai_viewer as viewer
from depthai_viewer._backend.device_configuration import CameraConfiguration
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


class CameraCallbackArgs(BaseModel):  # type: ignore[misc]
    board_socket: dai.CameraBoardSocket
    image_kind: dai.CameraSensorType

    class Config:
        arbitrary_types_allowed = True


class DepthCallbackArgs(BaseModel):  # type: ignore[misc]
    alignment_camera: CameraConfiguration
    stereo_pair: Tuple[dai.CameraBoardSocket, dai.CameraBoardSocket]

    class Config:
        arbitrary_types_allowed = True


class AiModelCallbackArgs(BaseModel):  # type: ignore[misc]
    model_name: str
    camera: CameraConfiguration
    labels: Optional[List[str]] = None

    class Config:
        arbitrary_types_allowed = True


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

    def build_callback(
        self, args: Union[CameraCallbackArgs, DepthCallbackArgs, AiModelCallbackArgs]
    ) -> Callable[[Any], None]:
        if isinstance(args, CameraCallbackArgs):
            return lambda packet: self._on_camera_frame(packet, args)  # type: ignore[arg-type]
        elif isinstance(args, DepthCallbackArgs):
            return lambda packet: self._on_stereo_frame(packet, args)  # type: ignore[arg-type]
        elif isinstance(args, AiModelCallbackArgs):
            callback: Callable[[Any, AiModelCallbackArgs], None] = self._on_detections
            if args.model_name == "age-gender-recognition-retail-0013":
                callback = self._on_age_gender_packet
            return lambda packet: callback(packet, args)  # type: ignore[arg-type]

    def _on_camera_frame(self, packet: FramePacket, args: CameraCallbackArgs) -> None:
        viewer.log_rigid3(f"{args.board_socket.name}/transform", child_from_parent=([0, 0, 0], self.ahrs.Q), xyz="RDF")
        h, w = packet.frame.shape[:2]
        child_from_parent: NDArray[np.float32]
        try:
            child_from_parent = self._get_camera_intrinsics(args.board_socket, w, h)
        except Exception:
            f_len = (w * h) ** 0.5
            child_from_parent = np.array([[f_len, 0, w / 2], [0, f_len, h / 2], [0, 0, 1]])
        viewer.log_pinhole(
            f"{args.board_socket.name}/transform/camera/",
            child_from_parent=child_from_parent,
            width=w,
            height=h,
        )
        img_frame = (
            packet.frame
            if args.image_kind == dai.CameraSensorType.MONO
            else cv2.cvtColor(packet.frame, cv2.COLOR_BGR2RGB)
        )
        viewer.log_image(f"{args.board_socket.name}/transform/camera/Image", img_frame)

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

    def _on_stereo_frame(self, frame: DepthPacket, args: DepthCallbackArgs) -> None:
        if Topic.DepthImage not in self.store.subscriptions:
            return
        depth_frame = frame.frame
        path = f"{args.alignment_camera.board_socket.name}/transform/camera" + "/Depth"
        if not self.store.pipeline_config or not self.store.pipeline_config.depth:
            # Essentially impossible to get here
            return
        viewer.log_depth_image(path, depth_frame, meter=1e3)

    def _on_detections(self, packet: DetectionPacket, args: AiModelCallbackArgs) -> None:
        rects, colors, labels = self._detections_to_rects_colors_labels(packet, args.labels)
        viewer.log_rects(
            f"{args.camera.board_socket.name}/transform/camera/Detections",
            rects,
            rect_format=RectFormat.XYXY,
            colors=colors,
            labels=labels,
        )

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

    def _on_age_gender_packet(self, packet: TwoStagePacket, args: AiModelCallbackArgs) -> None:
        for det, rec in zip(packet.detections, packet.nnData):
            age = int(float(np.squeeze(np.array(rec.getLayerFp16("age_conv3")))) * 100)
            gender = np.squeeze(np.array(rec.getLayerFp16("prob")))
            gender_str = "Woman" if gender[0] > gender[1] else "Man"
            label = f"{gender_str}, {age}"
            color = [255, 0, 0] if gender[0] > gender[1] else [0, 0, 255]
            # TODO(filip): maybe use viewer.log_annotation_context to log class colors for detections
            viewer.log_rect(
                f"{args.camera.board_socket.name}/transform/camera/Detection",
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
