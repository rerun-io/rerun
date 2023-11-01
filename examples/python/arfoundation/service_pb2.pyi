from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class RegisterRequest(_message.Message):
    __slots__ = ["focal_length_x", "focal_length_y", "principal_point_x", "principal_point_y", "color_resolution_x", "color_resolution_y", "color_sample_size_x", "color_sample_size_y", "depth_resolution_x", "depth_resolution_y"]
    FOCAL_LENGTH_X_FIELD_NUMBER: _ClassVar[int]
    FOCAL_LENGTH_Y_FIELD_NUMBER: _ClassVar[int]
    PRINCIPAL_POINT_X_FIELD_NUMBER: _ClassVar[int]
    PRINCIPAL_POINT_Y_FIELD_NUMBER: _ClassVar[int]
    COLOR_RESOLUTION_X_FIELD_NUMBER: _ClassVar[int]
    COLOR_RESOLUTION_Y_FIELD_NUMBER: _ClassVar[int]
    COLOR_SAMPLE_SIZE_X_FIELD_NUMBER: _ClassVar[int]
    COLOR_SAMPLE_SIZE_Y_FIELD_NUMBER: _ClassVar[int]
    DEPTH_RESOLUTION_X_FIELD_NUMBER: _ClassVar[int]
    DEPTH_RESOLUTION_Y_FIELD_NUMBER: _ClassVar[int]
    focal_length_x: float
    focal_length_y: float
    principal_point_x: float
    principal_point_y: float
    color_resolution_x: int
    color_resolution_y: int
    color_sample_size_x: int
    color_sample_size_y: int
    depth_resolution_x: int
    depth_resolution_y: int
    def __init__(self, focal_length_x: _Optional[float] = ..., focal_length_y: _Optional[float] = ..., principal_point_x: _Optional[float] = ..., principal_point_y: _Optional[float] = ..., color_resolution_x: _Optional[int] = ..., color_resolution_y: _Optional[int] = ..., color_sample_size_x: _Optional[int] = ..., color_sample_size_y: _Optional[int] = ..., depth_resolution_x: _Optional[int] = ..., depth_resolution_y: _Optional[int] = ...) -> None: ...

class RegisterResponse(_message.Message):
    __slots__ = ["message"]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    message: str
    def __init__(self, message: _Optional[str] = ...) -> None: ...

class DataFrameRequest(_message.Message):
    __slots__ = ["uid", "color", "depth", "transform"]
    UID_FIELD_NUMBER: _ClassVar[int]
    COLOR_FIELD_NUMBER: _ClassVar[int]
    DEPTH_FIELD_NUMBER: _ClassVar[int]
    TRANSFORM_FIELD_NUMBER: _ClassVar[int]
    uid: str
    color: bytes
    depth: bytes
    transform: bytes
    def __init__(self, uid: _Optional[str] = ..., color: _Optional[bytes] = ..., depth: _Optional[bytes] = ..., transform: _Optional[bytes] = ...) -> None: ...

class DataFrameResponse(_message.Message):
    __slots__ = ["message"]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    message: str
    def __init__(self, message: _Optional[str] = ...) -> None: ...
