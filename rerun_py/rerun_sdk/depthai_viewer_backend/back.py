import json
import threading
from queue import Empty as QueueEmptyException
from queue import Queue
from typing import Dict, Tuple
import time

import depthai as dai
import numpy as np
from depthai_sdk import OakCamera
from depthai_sdk.components import NNComponent

# from depthai_sdk.components.pointcloud_component import PointcloudComponent
from depthai_sdk.oak_camera import CameraComponent

from depthai_viewer_backend.config_api import start_api
from depthai_viewer_backend.device_configuration import PipelineConfiguration
from depthai_viewer_backend.sdk_callbacks import SdkCallbacks
from depthai_viewer_backend.store import Store

color_wh_to_enum = {
    (1280, 720): dai.ColorCameraProperties.SensorResolution.THE_720_P,
    (1280, 800): dai.ColorCameraProperties.SensorResolution.THE_800_P,
    (1920, 1080): dai.ColorCameraProperties.SensorResolution.THE_1080_P,
    (3840, 2160): dai.ColorCameraProperties.SensorResolution.THE_4_K,
    (4056, 3040): dai.ColorCameraProperties.SensorResolution.THE_12_MP,
    (1440, 1080): dai.ColorCameraProperties.SensorResolution.THE_1440X1080,
    (5312, 6000): dai.ColorCameraProperties.SensorResolution.THE_5312X6000,
    # TODO(filip): Add other resolutions
}

mono_wh_to_enum = {
    (640, 400): dai.MonoCameraProperties.SensorResolution.THE_400_P,
    (640, 480): dai.MonoCameraProperties.SensorResolution.THE_480_P,
    (1280, 720): dai.MonoCameraProperties.SensorResolution.THE_720_P,
    (1280, 800): dai.MonoCameraProperties.SensorResolution.THE_800_P,
    (1920, 1200): dai.MonoCameraProperties.SensorResolution.THE_1200_P,
}


class SelectedDevice:
    id: str
    intrinsic_matrix: Dict[Tuple[int, int], np.ndarray] = {}
    calibration_data: dai.CalibrationHandler = None
    use_encoding: bool = False

    _color: CameraComponent = None
    _left: CameraComponent = None
    _right: CameraComponent = None
    _stereo: CameraComponent = None
    _nnet: NNComponent = None
    # _pc: PointcloudComponent = None

    oak_cam: OakCamera = None

    def __init__(self, device_id: str):
        self.id = device_id
        self.oak_cam = OakCamera(self.id)
        print("Oak cam: ", self.oak_cam)

    def get_intrinsic_matrix(self, width: int, height: int) -> np.ndarray:
        if self.intrinsic_matrix.get((width, height)) is np.ndarray:
            return self.intrinsic_matrix.get((width, height))
        M_right = self.calibration_data.getCameraIntrinsics(dai.CameraBoardSocket.RIGHT, dai.Size2f(width, height))
        self.intrinsic_matrix[(width, height)] = np.array(M_right).reshape(3, 3)
        return self.intrinsic_matrix[(width, height)]

    def get_device_properties(self) -> Dict:
        dai_props = self.oak_cam.device.getConnectedCameraFeatures()
        device_properties = {
            "id": self.id,
            "supported_color_resolutions": [],
            "supported_left_mono_resolutions": [],
            "supported_right_mono_resolutions": [],
        }
        for cam in dai_props:
            resolutions_key = "supported_left_mono_resolutions"
            if cam.socket == dai.CameraBoardSocket.RGB:
                resolutions_key = "supported_color_resolutions"
            elif cam.socket == dai.CameraBoardSocket.RIGHT:
                resolutions_key = "supported_right_mono_resolutions"
            for config in cam.configs:
                wh = (config.width, config.height)
                if wh not in device_properties[resolutions_key]:
                    device_properties[resolutions_key].append((config.width, config.height))
        device_properties["supported_color_resolutions"] = list(
            map(
                lambda x: color_wh_to_enum[x].name,
                sorted(device_properties["supported_color_resolutions"], key=lambda x: x[0] * x[1]),
            )
        )
        device_properties["supported_left_mono_resolutions"] = list(
            map(
                lambda x: color_wh_to_enum[x].name,
                sorted(device_properties["supported_left_mono_resolutions"], key=lambda x: x[0] * x[1]),
            )
        )
        device_properties["supported_right_mono_resolutions"] = list(
            map(
                lambda x: color_wh_to_enum[x].name,
                sorted(device_properties["supported_right_mono_resolutions"], key=lambda x: x[0] * x[1]),
            )
        )
        return device_properties

    def update_pipeline(
        self, config: PipelineConfiguration, runtime_only: bool, callbacks: "SdkCallbacks"
    ) -> Tuple[bool, str]:
        if self.oak_cam.running():
            if runtime_only:
                return True, self._stereo.control.send_controls(config.depth.to_runtime_controls())
            print("Cam running, closing...")
            self.oak_cam.device.close()
            self.oak_cam = None
            # Check if the device is available, timeout after 10 seconds
            timeout_start = time.time()
            while time.time() - timeout_start < 10:
                available_devices = [device.getMxId() for device in dai.Device.getAllAvailableDevices()]
                if self.id in available_devices:
                    break
            try:
                self.oak_cam = OakCamera(self.id)
            except RuntimeError as e:
                print("Failed to create oak camera")
                print(e)
                self.oak_cam = None
                return False, {"message": "Failed to create oak camera"}

        self.use_encoding = self.oak_cam.device.getDeviceInfo().protocol == dai.XLinkProtocol.X_LINK_TCP_IP
        if self.use_encoding:
            print("Connected device is PoE: Using encoding...")
        else:
            print("Connected device is USB: Not using encoding...")
        if config.color_camera:
            print("Creating color camera")
            self._color = self.oak_cam.create_camera(
                "color", config.color_camera.resolution, config.color_camera.fps, name="color", encode=self.use_encoding
            )
            if config.color_camera.xout_video:
                self.oak_cam.callback(self._color, callbacks.on_color_frame, enable_visualizer=self.use_encoding)
        if config.left_camera:
            print("Creating left camera")
            self._left = self.oak_cam.create_camera(
                "left", config.left_camera.resolution, config.left_camera.fps, name="left", encode=self.use_encoding
            )
            if config.left_camera.xout:
                self.oak_cam.callback(self._left, callbacks.on_left_frame, enable_visualizer=self.use_encoding)
        if config.right_camera:
            print("Creating right camera")
            self._right = self.oak_cam.create_camera(
                "right", config.right_camera.resolution, config.right_camera.fps, name="right", encode=self.use_encoding
            )
            if config.right_camera.xout:
                self.oak_cam.callback(self._right, callbacks.on_right_frame, enable_visualizer=self.use_encoding)
        if config.depth:
            print("Creating depth")
            self._stereo = self.oak_cam.create_stereo(left=self._left, right=self._right, name="depth")
            self._stereo.config_stereo(
                lr_check=config.depth.lr_check,
                subpixel=config.depth.subpixel_disparity,
                confidence=config.depth.confidence,
                align=config.depth.align,
                lr_check_threshold=config.depth.lrc_threshold,
                median=config.depth.median,
            )
            self.oak_cam.callback(self._stereo, callbacks.on_stereo_frame)
            # if config.depth.pointcloud and config.depth.pointcloud.enabled:
            #     self._pc = self.oak_cam.create_pointcloud(stereo=self._stereo, colorize=self._color)
            #     self.oak_cam.callback(self._pc, callbacks.on_pointcloud)

        if config.imu:
            print("Creating IMU")
            imu = self.oak_cam.create_imu()
            sensors = [
                dai.IMUSensor.ACCELEROMETER_RAW,
                dai.IMUSensor.GYROSCOPE_CALIBRATED,
            ]
            if "BNO" in self.oak_cam.device.getConnectedIMU():
                sensors.append(dai.IMUSensor.MAGNETOMETER_CALIBRATED)
            imu.config_imu(
                sensors, report_rate=config.imu.report_rate, batch_report_threshold=config.imu.batch_report_threshold
            )
            self.oak_cam.callback(imu, callbacks.on_imu)

        if config.ai_model and config.ai_model.path:
            if config.ai_model.path == "age-gender-recognition-retail-0013":
                face_detection = self.oak_cam.create_nn("face-detection-retail-0004", self._color)
                self._nnet = self.oak_cam.create_nn("age-gender-recognition-retail-0013", input=face_detection)
                self.oak_cam.callback(self._nnet, callbacks.on_age_gender_packet)
            elif config.ai_model.path == "mobilenet-ssd":
                self._nnet = self.oak_cam.create_nn(
                    config.ai_model.path,
                    self._color,
                )
                self.oak_cam.callback(self._nnet, callbacks.on_mobilenet_ssd_packet)
            else:
                self._nnet = self.oak_cam.create_nn(config.ai_model.path, self._color)
                callback = callbacks.on_detections
                if config.ai_model.path == "yolov8n_coco_640x352":
                    callback = callbacks.on_yolo_packet
                self.oak_cam.callback(self._nnet, callback)
        try:
            self.oak_cam.start(blocking=False)
        except RuntimeError as e:
            print("Couldn't start pipeline: ", e)
            return False, {"message": "Couldn't start pipeline"}
        running = self.oak_cam.running()
        if running:
            self.oak_cam.poll()
            self.calibration_data = self.oak_cam.device.readCalibration()
            self.intrinsic_matrix = {}
        return running, {"message": "Pipeline started" if running else "Couldn't start pipeline"}


class DepthaiViewerBack:
    _device: SelectedDevice = None

    # Queues for communicating with the API process
    action_queue: Queue
    result_queue: Queue

    # Sdk callbacks for handling data from the device and sending it to the frontend
    sdk_callbacks: SdkCallbacks

    def __init__(self, compression: bool = False) -> None:
        self.action_queue = Queue()
        self.result_queue = Queue()
        self.send_message_queue = Queue()

        self.store = Store()
        self.store.on_update_pipeline = self.update_pipeline
        self.store.on_select_device = self.select_device
        self.store.on_reset = self.on_reset

        self.api_process = threading.Thread(
            target=start_api, args=(self.action_queue, self.result_queue, self.send_message_queue)
        )
        self.api_process.start()

        self.sdk_callbacks = SdkCallbacks(self.store)
        self.run()

    def set_device(self, device: SelectedDevice | None):
        self._device = device
        if device:
            self.sdk_callbacks.set_camera_intrinsics_getter(device.get_intrinsic_matrix)

    def on_reset(self) -> Tuple[bool, str]:
        print("Resetting...")
        if self._device:
            print("Closing device...")
            self._device.oak_cam.device.close()
            self._device.oak_cam.__exit__(None, None, None)
            self._device.oak_cam = None
            self.set_device(None)
        print("Done")
        return True, {"message": "Reset successful"}

    def select_device(self, device_id: str) -> Tuple[bool, str]:
        print("Selecting device: ", device_id)
        if self._device:
            self.on_reset()
        if device_id == "":
            return True, {"message": "Successfully unselected device", "device_properties": {}}
        try:
            self.set_device(SelectedDevice(device_id))
        except RuntimeError as e:
            print("Failed to select device:", e)
            return False, {"message": "Failed to select device", "device_properties": {}}
        try:
            device_properties = self._device.get_device_properties()
            return True, {"message:": "Device selected successfully", "device_properties": device_properties}
        except RuntimeError as e:
            print("Failed to get device properties:", e)
            self.on_reset()
            print("Restarting backend...")
            # For now exit the backend, the frontend will restart it
            # (TODO(filip): Why does "Device already closed or disconnected: Input/output error happen")
            exit(-1)
            # return False, {"message": "Failed to get device properties", "device_properties": {}}

    def update_pipeline(self, runtime_only: bool) -> bool:
        if not self._device:
            print("No device selected, can't update pipeline!")
            return False, {"message": "No device selected, can't update pipeline!"}
        print("Updating pipeline...")
        started, message = self._device.update_pipeline(
            self.store.pipeline_config, runtime_only, callbacks=self.sdk_callbacks
        )
        if not started:
            self.set_device(None)
        return started, {"message": message}

    def run(self):
        """Handles ws messages and poll OakCam."""
        while True:
            try:
                action, kwargs = self.action_queue.get(timeout=0.001)
                print("Handling action: ", action)
                self.result_queue.put(self.store.handle_action(action, **kwargs))
            except QueueEmptyException:
                pass

            if self._device and self._device.oak_cam:
                self._device.oak_cam.poll()
                if self._device.oak_cam.device.isClosed():
                    # TODO(filip): Typehint the messages properly
                    self.on_reset()
                    self.send_message_queue.put(
                        json.dumps({"type": "Error", "data": {"action": "FullReset", "message": "Device disconnected"}})
                    )


if __name__ == "__main__":
    back = DepthaiViewerBack()
