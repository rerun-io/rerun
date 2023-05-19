from typing import Any, Dict, Optional

import depthai as dai
from depthai_sdk import Previews as QueueNames
from pydantic import BaseModel


class ColorCameraConfiguration(BaseModel):  # type: ignore[misc]
    fps: Optional[int] = 30
    resolution: Optional[
        dai.ColorCameraProperties.SensorResolution
    ] = dai.ColorCameraProperties.SensorResolution.THE_1080_P
    board_socket: Optional[dai.CameraBoardSocket] = dai.CameraBoardSocket.RGB
    out_preview: bool = False
    xout_still: bool = False
    xout_video: bool = True
    input_control: bool = False

    class Config:
        arbitrary_types_allowed = True
        # Doesnt work atm
        json_encoders = {
            Optional[dai.MonoCameraProperties.SensorResolution]: lambda v: v.name,
            dai.CameraBoardSocket: lambda v: v.name,
        }

    def __init__(self, **v) -> None:  # type: ignore[no-untyped-def]
        if v.get("resolution"):
            v["resolution"] = getattr(dai.ColorCameraProperties.SensorResolution, v["resolution"])
        if v.get("board_socket"):
            v["board_socket"] = getattr(dai.CameraBoardSocket, v["board_socket"])
        return super().__init__(**v)  # type: ignore[no-any-return]

    @property
    # Make this select the queue based on ui, also probably not just one queue
    def out_queue_name(self) -> Optional[str]:
        prefix: str = QueueNames.color.name
        if self.out_preview:
            return prefix + "_preview"
        if self.xout_still:
            return prefix + "_still"
        if self.xout_video:
            return prefix + "_video"
        return None


class MonoCameraConfiguration(BaseModel):  # type: ignore[misc]
    fps: Optional[int] = 30
    resolution: Optional[
        dai.MonoCameraProperties.SensorResolution
    ] = dai.MonoCameraProperties.SensorResolution.THE_400_P
    board_socket: Optional[dai.CameraBoardSocket] = dai.CameraBoardSocket.LEFT
    xout: bool = False  # Depth queue fails if I create this queue!
    input_control: bool = False

    class Config:
        arbitrary_types_allowed = True
        # Doesnt work atm
        json_encoders = {
            Optional[dai.MonoCameraProperties.SensorResolution]: lambda v: v.name,
            dai.CameraBoardSocket: lambda v: v.name,
        }

    def __init__(self, **v) -> None:  # type: ignore[no-untyped-def]
        if v.get("resolution"):
            v["resolution"] = getattr(dai.MonoCameraProperties.SensorResolution, v["resolution"])
        if v.get("board_socket"):
            v["board_socket"] = getattr(dai.CameraBoardSocket, v["board_socket"])
        return super().__init__(**v)  # type: ignore[no-any-return]

    @property
    def out_queue_name(self) -> str:
        return "left" if self.board_socket == dai.CameraBoardSocket.LEFT else "right"

    @classmethod
    def create_left(cls, **kwargs) -> "MonoCameraConfiguration":  # type: ignore[no-untyped-def]
        return cls(board_socket="LEFT", **kwargs)

    @classmethod
    def create_right(cls, **kwargs) -> "MonoCameraConfiguration":  # type: ignore[no-untyped-def]
        return cls(board_socket="RIGHT", **kwargs)


# class PointcloudConfiguration(BaseModel):
#     enabled: bool = True


class DepthConfiguration(BaseModel):  # type: ignore[misc]
    median: Optional[dai.MedianFilter] = dai.MedianFilter.KERNEL_7x7
    lr_check: Optional[bool] = True
    lrc_threshold: int = 5  # 0..10
    extended_disparity: Optional[bool] = False
    subpixel_disparity: Optional[bool] = True
    align: Optional[dai.CameraBoardSocket] = dai.CameraBoardSocket.CENTER
    sigma: int = 0  # 0..65535
    # pointcloud: PointcloudConfiguration | None = None
    confidence: int = 230

    class Config:
        arbitrary_types_allowed = True

    def __init__(self, **v) -> None:  # type: ignore[no-untyped-def]
        if v.get("median"):
            v["median"] = getattr(dai.MedianFilter, v["median"])
        if v.get("align"):
            v["align"] = getattr(dai.CameraBoardSocket, v["align"])

        return super().__init__(**v)  # type: ignore[no-any-return]

    def to_runtime_controls(self) -> Dict[str, Any]:
        return {
            "algorithm_control": {
                "align": "RECTIFIED_LEFT"
                if self.align == dai.CameraBoardSocket.LEFT
                else "RECTIFIED_RIGHT"
                if self.align == dai.CameraBoardSocket.RIGHT
                else "CENTER",
                "lr_check": self.lr_check,
                "lrc_check_threshold": self.lrc_threshold,
                "extended": self.extended_disparity,
                "subpixel": self.subpixel_disparity,
            },
            "postprocessing": {
                "median": {
                    dai.MedianFilter.MEDIAN_OFF: 0,
                    dai.MedianFilter.KERNEL_3x3: 3,
                    dai.MedianFilter.KERNEL_5x5: 5,
                    dai.MedianFilter.KERNEL_7x7: 7,
                }[self.median]
                if self.median
                else 0,
                "bilateral_sigma": self.sigma,
            },
            "cost_matching": {
                "confidence_threshold": self.confidence,
            },
        }

    @property
    def out_queue_name(self) -> str:
        return str(QueueNames.depthRaw.name)


class AiModelConfiguration(BaseModel):  # type: ignore[misc]
    display_name: str = "Yolo V8"
    path: str = "yolov8n_coco_640x352"


class ImuConfiguration(BaseModel):  # type: ignore[misc]
    report_rate: int = 100
    batch_report_threshold: int = 5


class PipelineConfiguration(BaseModel):  # type: ignore[misc]
    color_camera: ColorCameraConfiguration = ColorCameraConfiguration()
    left_camera: MonoCameraConfiguration = MonoCameraConfiguration.create_left()
    right_camera: MonoCameraConfiguration = MonoCameraConfiguration.create_right()
    depth: Optional[DepthConfiguration] = DepthConfiguration()
    ai_model: Optional[AiModelConfiguration] = AiModelConfiguration()
    imu: ImuConfiguration = ImuConfiguration()

    def to_json(self) -> Dict[str, Any]:
        as_dict = self.dict()
        return self._fix_depthai_types(as_dict)

    def _fix_depthai_types(self, as_dict: Dict[str, Any]) -> Dict[str, Any]:
        """ATM Config.json_encoders doesn't work, so we manually fix convert the depthai types to strings here."""
        if as_dict.get("color_camera"):
            as_dict["color_camera"] = self._fix_camera(as_dict["color_camera"])
        if as_dict.get("left_camera"):
            as_dict["left_camera"] = self._fix_camera(as_dict["left_camera"])
        if as_dict.get("right_camera"):
            as_dict["right_camera"] = self._fix_camera(as_dict["right_camera"])
        if as_dict.get("depth"):
            as_dict["depth"] = self._fix_depth(as_dict["depth"])
        return as_dict

    def _fix_depth(self, as_dict: Dict[str, Any]) -> Dict[str, Any]:
        if as_dict.get("align"):
            as_dict["align"] = as_dict["align"].name
        if as_dict.get("median"):
            as_dict["median"] = as_dict["median"].name
        return as_dict

    def _fix_camera(self, as_dict: Dict[str, Any]) -> Dict[str, Any]:
        if as_dict.get("resolution"):
            as_dict["resolution"] = as_dict["resolution"].name
        if as_dict.get("board_socket"):
            as_dict["board_socket"] = as_dict["board_socket"].name
        return as_dict
