import json
import threading
import time
from queue import Empty as QueueEmptyException
from queue import Queue
from typing import Any, Dict, List, Optional, Tuple, Union
import itertools

import depthai as dai
import depthai_sdk
import numpy as np
import pkg_resources
from depthai_sdk import OakCamera
from depthai_sdk.components import CameraComponent, NNComponent, StereoComponent
from numpy.typing import NDArray

import depthai_viewer as viewer
from depthai_viewer._backend.config_api import start_api
from depthai_viewer._backend.device_configuration import (
    CameraFeatures,
    DeviceProperties,
    PipelineConfiguration,
    ImuKind,
    CameraConfiguration,
    compare_dai_camera_configs,
    resolution_to_enum,
    calculate_isp_scale,
)
from depthai_viewer._backend.sdk_callbacks import *
from depthai_viewer._backend.store import Store
from depthai_viewer._backend import classification_labels

viewer.init("Depthai Viewer")
viewer.connect()


class SelectedDevice:
    id: str
    intrinsic_matrix: Dict[Tuple[dai.CameraBoardSocket, int, int], NDArray[np.float32]] = {}
    calibration_data: Optional[dai.CalibrationHandler] = None
    use_encoding: bool = False
    _time_of_last_xlink_update: int = 0
    _cameras: List[CameraComponent] = []
    _stereo: StereoComponent = None
    _nnet: NNComponent = None
    # _pc: PointcloudComponent = None

    oak_cam: OakCamera = None

    def __init__(self, device_id: str):
        self.id = device_id
        self.oak_cam = OakCamera(self.id)
        print("Oak cam: ", self.oak_cam)

    def get_intrinsic_matrix(self, board_socket: dai.CameraBoardSocket, width: int, height: int) -> NDArray[np.float32]:
        if self.intrinsic_matrix.get((board_socket, width, height)) is not None:
            return self.intrinsic_matrix.get((board_socket, width, height))  # type: ignore[return-value]
        if self.calibration_data is None:
            raise Exception("Missing calibration data!")
        M_right = self.calibration_data.getCameraIntrinsics(  # type: ignore[union-attr]
            board_socket, dai.Size2f(width, height)
        )
        self.intrinsic_matrix[(board_socket, width, height)] = np.array(M_right).reshape(3, 3)
        return self.intrinsic_matrix[(board_socket, width, height)]

    def _get_possible_stereo_pairs_for_cam(
        self, cam: dai.CameraFeatures, connected_camera_features: List[dai.CameraFeatures]
    ) -> List[dai.CameraBoardSocket]:
        """
        Tries to find the possible stereo pairs for a camera.
        """
        stereo_pairs = []
        if cam.name == "right":
            stereo_pairs.extend(
                [features.socket for features in filter(lambda c: c.name == "left", connected_camera_features)]
            )
        elif cam.name == "left":
            stereo_pairs.extend(
                [features.socket for features in filter(lambda c: c.name == "right", connected_camera_features)]
            )
        else:
            stereo_pairs.extend(
                [
                    camera.socket
                    for camera in connected_camera_features
                    if camera != cam
                    and all(
                        map(
                            lambda confs: compare_dai_camera_configs(confs[0], confs[1]),
                            zip(camera.configs, cam.configs),
                        )
                    )
                ]
            )
        return stereo_pairs

    def get_device_properties(self) -> DeviceProperties:
        connected_cam_features = self.oak_cam.device.getConnectedCameraFeatures()
        imu = self.oak_cam.device.getConnectedIMU()
        imu = ImuKind.NINE_AXIS if "BNO" in imu else None if imu == "NONE" else ImuKind.SIX_AXIS
        device_properties = DeviceProperties(id=self.id, imu=imu)
        for cam in connected_cam_features:
            device_properties.cameras.append(
                CameraFeatures(
                    board_socket=cam.socket,
                    max_fps=60,
                    resolutions=[resolution_to_enum[(conf.width, conf.height)] for conf in cam.configs],
                    supported_types=cam.supportedTypes,
                    stereo_pairs=self._get_possible_stereo_pairs_for_cam(cam, connected_cam_features),
                    name=cam.name.capitalize(),
                )
            )
        device_properties.stereo_pairs = list(
            itertools.chain.from_iterable(
                [(cam.board_socket, pair) for pair in cam.stereo_pairs] for cam in device_properties.cameras
            )
        )
        return device_properties

    def close_oak_cam(self) -> None:
        if self.oak_cam.running():
            self.oak_cam.device.__exit__(0, 0, 0)

    def reconnect_to_oak_cam(self) -> Tuple[bool, Dict[str, str]]:
        """

        Try to reconnect to the device with self.id.

        Timeout after 10 seconds.
        """
        if self.oak_cam.device.isClosed():
            timeout_start = time.time()
            while time.time() - timeout_start < 10:
                available_devices = [
                    device.getMxId() for device in dai.Device.getAllAvailableDevices()  # type: ignore[call-arg]
                ]
                if self.id in available_devices:
                    break
            try:
                self.oak_cam = OakCamera(self.id)
                return True, {"message": "Successfully reconnected to device"}
            except RuntimeError as e:
                print("Failed to create oak camera")
                print(e)
                self.oak_cam = None
        return False, {"message": "Failed to create oak camera"}

    def _get_component_by_socket(self, socket: dai.CameraBoardSocket) -> Optional[CameraComponent]:
        component = list(filter(lambda c: c.node.getBoardSocket() == socket, self._cameras))
        if not component:
            return None
        return component[0]

    def _get_camera_config_by_socket(
        self, config: PipelineConfiguration, socket: dai.CameraBoardSocket
    ) -> Optional[CameraConfiguration]:
        print("Getting cam by socket: ", socket, " Cameras: ", config.cameras)
        camera = list(filter(lambda c: c.board_socket == socket, config.cameras))
        if not camera:
            return None
        return camera[0]

    def update_pipeline(
        self, config: PipelineConfiguration, runtime_only: bool, callbacks: "SdkCallbacks"
    ) -> Tuple[bool, Dict[str, str]]:
        if self.oak_cam.running():
            if runtime_only:
                if config.depth is not None:
                    return True, self._stereo.control.send_controls(config.depth.to_runtime_controls())
                return False, {"message": "Depth is not enabled, can't send runtime controls!"}
            print("Cam running, closing...")
            self.close_oak_cam()
            success, message = self.reconnect_to_oak_cam()
            if not success:
                return success, message

        self._cameras = []
        self.use_encoding = self.oak_cam.device.getDeviceInfo().protocol == dai.XLinkProtocol.X_LINK_TCP_IP
        if self.use_encoding:
            print("Connected device is PoE: Using encoding...")
        else:
            print("Connected device is USB: Not using encoding...")
        for cam in config.cameras:
            print("Creating camera: ", cam)
            sdk_cam = self.oak_cam.create_camera(
                cam.board_socket,
                cam.resolution.as_sdk_resolution(),
                cam.fps,
                encode=self.use_encoding,
            )
            if cam.stream_enabled:
                callback_args = CameraCallbackArgs(board_socket=cam.board_socket, image_kind=cam.kind)
                self.oak_cam.callback(
                    sdk_cam, callbacks.build_callback(callback_args), enable_visualizer=self.use_encoding
                )
            self._cameras.append(sdk_cam)

        if config.depth:
            print("Creating depth")
            stereo_pair = config.depth.stereo_pair
            left_cam = self._get_component_by_socket(stereo_pair[0])
            right_cam = self._get_component_by_socket(stereo_pair[1])
            if not left_cam or not right_cam:
                return False, {"message": f"{cam} is not configured. Couldn't create stereo pair."}

            if left_cam.node.getResolutionWidth() > 1280:
                print("Left cam width > 1280, setting isp scale to get 800")
                left_cam.config_color_camera(isp_scale=calculate_isp_scale(left_cam.node.getResolutionWidth()))
            if right_cam.node.getResolutionWidth() > 1280:
                print("Right cam width > 1280, setting isp scale to get 800")
                right_cam.config_color_camera(isp_scale=calculate_isp_scale(right_cam.node.getResolutionWidth()))
            self._stereo = self.oak_cam.create_stereo(left=left_cam, right=right_cam, name="depth")

            # We used to be able to pass in the board socket to align to, but this was removed in depthai 1.10.0
            align = config.depth.align
            if pkg_resources.parse_version(depthai_sdk.__version__) >= pkg_resources.parse_version("1.10.0"):
                align_component = self._get_component_by_socket(align)
                if not align_component:
                    return False, {"message": f"{config.depth.align} is not configured. Couldn't create stereo pair."}
                align = align_component
            self._stereo.config_stereo(
                lr_check=config.depth.lr_check,
                subpixel=config.depth.subpixel_disparity,
                confidence=config.depth.confidence,
                align=align,
                lr_check_threshold=config.depth.lrc_threshold,
                median=config.depth.median,
            )

            aligned_camera = self._get_camera_config_by_socket(config, config.depth.align)
            if not aligned_camera:
                return False, {"message": f"{config.depth.align} is not configured. Couldn't create stereo pair."}
            self.oak_cam.callback(
                self._stereo,
                callbacks.build_callback(
                    DepthCallbackArgs(alignment_camera=aligned_camera, stereo_pair=config.depth.stereo_pair)
                ),
            )

        if self.oak_cam.device.getConnectedIMU() != "NONE":
            print("Creating IMU")
            imu = self.oak_cam.create_imu()
            sensors = [
                dai.IMUSensor.ACCELEROMETER_RAW,
                dai.IMUSensor.GYROSCOPE_RAW,
            ]
            if "BNO" in self.oak_cam.device.getConnectedIMU():
                sensors.append(dai.IMUSensor.MAGNETOMETER_CALIBRATED)
            imu.config_imu(
                sensors, report_rate=config.imu.report_rate, batch_report_threshold=config.imu.batch_report_threshold
            )
            self.oak_cam.callback(imu, callbacks.on_imu)
        else:
            print("Connected cam doesn't have IMU, skipping IMU creation...")

        if config.ai_model and config.ai_model.path:
            cam_component = self._get_component_by_socket(config.ai_model.camera)
            if not cam_component:
                return False, {"message": f"{config.ai_model.camera} is not configured."}
            labels: Optional[List[str]] = None
            if config.ai_model.path == "age-gender-recognition-retail-0013":
                face_detection = self.oak_cam.create_nn("face-detection-retail-0004", cam_component)
                self._nnet = self.oak_cam.create_nn("age-gender-recognition-retail-0013", input=face_detection)

            else:
                self._nnet = self.oak_cam.create_nn(config.ai_model.path, cam_component)
                labels = getattr(classification_labels, config.ai_model.path.upper().replace("-", "_"), None)

            camera = self._get_camera_config_by_socket(config, config.ai_model.camera)
            if not camera:
                return False, {"message": f"{config.ai_model.camera} is not configured. Couldn't create NN."}
            self.oak_cam.callback(
                self._nnet,
                callbacks.build_callback(
                    AiModelCallbackArgs(model_name=config.ai_model.path, camera=camera, labels=labels)
                ),
                True,
            )  # in depthai-sdk=1.10.0 nnet callbacks don't work without visualizer enabled
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

    def update(self) -> None:
        self.oak_cam.poll()
        if time.time_ns() - self._time_of_last_xlink_update >= 16e6:
            self._time_of_last_xlink_update = time.time_ns()
            if hasattr(self.oak_cam.device, "getProfilingData"):  # Only on latest develop
                xlink_stats = self.oak_cam.device.getProfilingData()
                viewer.log_xlink_stats(xlink_stats.numBytesWritten, xlink_stats.numBytesRead)


class DepthaiViewerBack:
    _device: Optional[SelectedDevice] = None

    # Queues for communicating with the API process
    action_queue: Queue  # type: ignore[type-arg]
    result_queue: Queue  # type: ignore[type-arg]
    send_message_queue: Queue  # type: ignore[type-arg]

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

    def set_device(self, device: Optional[SelectedDevice] = None) -> None:
        self._device = device
        if device:
            self.sdk_callbacks.set_camera_intrinsics_getter(device.get_intrinsic_matrix)

    def on_reset(self) -> Tuple[bool, Dict[str, str]]:
        print("Resetting...")
        if self._device:
            print("Closing device...")
            self._device.close_oak_cam()
            self.set_device(None)
        print("Done")
        return True, {"message": "Reset successful"}

    def select_device(self, device_id: str) -> Tuple[bool, Dict[str, Union[str, Any]]]:
        print("Selecting device: ", device_id)
        if self._device:
            self.on_reset()
        if device_id == "":
            return True, {"message": "Successfully unselected device", "device_properties": {}}
        try:
            self.set_device(SelectedDevice(device_id))
        except RuntimeError as e:
            print("Failed to select device:", e)
            return False, {
                "message": str(e) + ", Try plugging in the device on a different port.",
                "device_properties": {},
            }
        try:
            if self._device is not None:
                device_properties = self._device.get_device_properties()
                return True, {"message:": "Device selected successfully", "device_properties": device_properties}
            return False, {"message": "CCouldn't select device", "device_properties": {}}
        except RuntimeError as e:
            print("Failed to get device properties:", e)
            self.on_reset()
            self.send_message_queue.put(
                json.dumps({"type": "Error", "data": {"action": "FullReset", "message": "Device disconnected"}})
            )
            print("Restarting backend...")
            # For now exit the backend, the frontend will restart it
            # (TODO(filip): Why does "Device already closed or disconnected: Input/output error happen")
            exit(-1)
            # return False, {"message": "Failed to get device properties", "device_properties": {}}

    def update_pipeline(self, runtime_only: bool) -> Tuple[bool, Dict[str, str]]:
        if not self._device:
            print("No device selected, can't update pipeline!")
            return False, {"message": "No device selected, can't update pipeline!"}
        print("Updating pipeline...")
        started, message = False, {"message": "Couldn't start pipeline"}
        if self.store.pipeline_config is not None:
            started, message = self._device.update_pipeline(
                self.store.pipeline_config, runtime_only, callbacks=self.sdk_callbacks
            )
        return started, message

    def run(self) -> None:
        """Handles ws messages and polls OakCam."""
        while True:
            try:
                action, kwargs = self.action_queue.get(timeout=0.0001)
                print("Handling action: ", action)
                self.result_queue.put(self.store.handle_action(action, **kwargs))
            except QueueEmptyException:
                pass

            if self._device:
                self._device.update()
                if self._device.oak_cam.device.isClosed():
                    # TODO(filip): Typehint the messages properly
                    self.on_reset()
                    self.send_message_queue.put(
                        json.dumps({"type": "Error", "data": {"action": "FullReset", "message": "Device disconnected"}})
                    )


if __name__ == "__main__":
    viewer.spawn(connect=True)
    DepthaiViewerBack()
