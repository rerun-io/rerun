# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/video_timestamp.fbs".

# You can extend this class by creating a "VideoTimeModeExt" class in "video_time_mode_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)

__all__ = ["VideoTimeMode", "VideoTimeModeArrayLike", "VideoTimeModeBatch", "VideoTimeModeLike", "VideoTimeModeType"]


from enum import Enum


class VideoTimeMode(Enum):
    """**Datatype**: Specifies how to interpret the `video_time` field of a [`datatypes.VideoTimestamp`][rerun.datatypes.VideoTimestamp]."""

    Nanoseconds = 1
    """Presentation timestamp in nanoseconds since the beginning of the video."""

    @classmethod
    def auto(cls, val: str | int | VideoTimeMode) -> VideoTimeMode:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, VideoTimeMode):
            return val
        if isinstance(val, int):
            return cls(val)
        try:
            return cls[val]
        except KeyError:
            val_lower = val.lower()
            for variant in cls:
                if variant.name.lower() == val_lower:
                    return variant
        raise ValueError(f"Cannot convert {val} to {cls.__name__}")

    def __str__(self) -> str:
        """Returns the variant name."""
        return self.name


VideoTimeModeLike = Union[VideoTimeMode, Literal["Nanoseconds", "nanoseconds"], int]
VideoTimeModeArrayLike = Union[VideoTimeModeLike, Sequence[VideoTimeModeLike]]


class VideoTimeModeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.VideoTimeMode"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class VideoTimeModeBatch(BaseBatch[VideoTimeModeArrayLike]):
    _ARROW_TYPE = VideoTimeModeType()

    @staticmethod
    def _native_to_pa_array(data: VideoTimeModeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (VideoTimeMode, int, str)):
            data = [data]

        pa_data = [VideoTimeMode.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
