from typing import Optional

import depthai as dai
from depthai_sdk.previews import Previews as QueueNames
from pydantic import BaseModel


class ColorCameraConfiguration(BaseModel):
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

    def __init__(self, **v):
        if v.get("resolution"):
            v["resolution"] = getattr(dai.ColorCameraProperties.SensorResolution, v["resolution"])
        if v.get("board_socket"):
            v["board_socket"] = getattr(dai.CameraBoardSocket, v["board_socket"])
        return super().__init__(**v)

    @property
    # Make this select the queue based on ui, also probably not just one queue
    def out_queue_name(self) -> str | None:
        prefix = QueueNames.color.name
        if self.out_preview:
            return prefix + "_preview"
        if self.xout_still:
            return prefix + "_still"
        if self.xout_video:
            return prefix + "_video"


class MonoCameraConfiguration(BaseModel):
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

    def __init__(self, **v):
        if v.get("resolution"):
            v["resolution"] = getattr(dai.MonoCameraProperties.SensorResolution, v["resolution"])
        if v.get("board_socket"):
            v["board_socket"] = getattr(dai.CameraBoardSocket, v["board_socket"])
        return super().__init__(**v)

    @property
    def out_queue_name(self) -> str:
        return "left" if self.board_socket == dai.CameraBoardSocket.LEFT else "right"

    @classmethod
    def create_left(cls, **kwargs):
        return cls(board_socket="LEFT", **kwargs)

    @classmethod
    def create_right(cls, **kwargs):
        return cls(board_socket="RIGHT", **kwargs)


# class PointcloudConfiguration(BaseModel):
#     enabled: bool = True


class DepthConfiguration(BaseModel):
    median: Optional[dai.StereoDepthProperties.MedianFilter] = dai.StereoDepthProperties.MedianFilter.KERNEL_7x7
    lr_check: Optional[bool] = True
    lrc_threshold: int = 5  # 0..10
    extended_disparity: Optional[bool] = False
    subpixel_disparity: Optional[bool] = True
    align: Optional[dai.CameraBoardSocket] = dai.CameraBoardSocket.RGB
    sigma: int = 0  # 0..65535
    # pointcloud: PointcloudConfiguration | None = None
    confidence: int = 230

    class Config:
        arbitrary_types_allowed = True

    def __init__(self, **v):
        if v.get("median"):
            v["median"] = getattr(dai.StereoDepthProperties.MedianFilter, v["median"])
        if v.get("align"):
            v["align"] = getattr(dai.CameraBoardSocket, v["align"])
        return super().__init__(**v)

    def requires_rebuild(self, other: "DepthConfiguration") -> bool:
        dont_require_rebuild = {"lrc_threshold", "sigma", "dct", "median"}
        return len(dont_require_rebuild - self._get_modified_fields(other)) != 0

    def _get_modified_fields(self, other: "DepthConfiguration") -> set[str]:
        fields = set()
        if self.dct != other.dct:
            fields.add("dct")
        if self.median != other.median:
            fields.add("median")
        if self.lr_check != other.lr_check:
            fields.add("lr_check")
        if self.lrc_threshold != other.lrc_threshold:
            fields.add("lrc_threshold")
        if self.extended_disparity != other.extended_disparity:
            fields.add("extended_disparity")
        if self.subpixel_disparity != other.subpixel_disparity:
            fields.add("subpixel_disparity")
        if self.align != other.align:
            fields.add("align")
        if self.sigma != other.sigma:
            fields.add("sigma")
        return fields

    @property
    def out_queue_name(self) -> str:
        return QueueNames.depthRaw.name


class AiModelConfiguration(BaseModel):
    display_name: str
    path: str


class ImuConfiguration(BaseModel):
    report_rate: int = 100
    batch_report_threshold: int = 5


class PipelineConfiguration(BaseModel):
    color_camera: ColorCameraConfiguration = ColorCameraConfiguration()
    left_camera: MonoCameraConfiguration = MonoCameraConfiguration.create_left()
    right_camera: MonoCameraConfiguration = MonoCameraConfiguration.create_right()
    depth: DepthConfiguration | None
    ai_model: AiModelConfiguration | None
    imu: ImuConfiguration = ImuConfiguration()

    def to_json(self):
        as_dict = self.dict()
        return self._fix_depthai_types(as_dict)

    def _fix_depthai_types(self, as_dict: dict):
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

    def _fix_depth(self, as_dict: dict):
        if as_dict.get("align"):
            as_dict["align"] = as_dict["align"].name
        if as_dict.get("median"):
            as_dict["median"] = as_dict["median"].name
        return as_dict

    def _fix_camera(self, as_dict: dict):
        if as_dict.get("resolution"):
            as_dict["resolution"] = as_dict["resolution"].name
        if as_dict.get("board_socket"):
            as_dict["board_socket"] = as_dict["board_socket"].name
        return as_dict
