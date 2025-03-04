# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/video_timestamp.fbs".

# You can extend this class by creating a "VideoTimestampExt" class in "video_timestamp_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)

__all__ = ["VideoTimestamp", "VideoTimestampArrayLike", "VideoTimestampBatch", "VideoTimestampLike"]


@define(init=False)
class VideoTimestamp:
    """
    **Datatype**: Presentation timestamp within a [`archetypes.AssetVideo`][rerun.archetypes.AssetVideo].

    Specified in nanoseconds.
    Presentation timestamps are typically measured as time since video start.
    """

    def __init__(self: Any, timestamp_ns: VideoTimestampLike):
        """
        Create a new instance of the VideoTimestamp datatype.

        Parameters
        ----------
        timestamp_ns:
            Presentation timestamp value in nanoseconds.

        """

        # You can define your own __init__ function as a member of VideoTimestampExt in video_timestamp_ext.py
        self.__attrs_init__(timestamp_ns=timestamp_ns)

    timestamp_ns: int = field(converter=int)
    # Presentation timestamp value in nanoseconds.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of VideoTimestampExt in video_timestamp_ext.py
        return np.asarray(self.timestamp_ns, dtype=dtype, copy=copy)

    def __int__(self) -> int:
        return int(self.timestamp_ns)

    def __hash__(self) -> int:
        return hash(self.timestamp_ns)


if TYPE_CHECKING:
    VideoTimestampLike = Union[VideoTimestamp, int]
else:
    VideoTimestampLike = Any

VideoTimestampArrayLike = Union[VideoTimestamp, Sequence[VideoTimestampLike], npt.NDArray[np.int64]]


class VideoTimestampBatch(BaseBatch[VideoTimestampArrayLike]):
    _ARROW_DATATYPE = pa.int64()

    @staticmethod
    def _native_to_pa_array(data: VideoTimestampArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.int64).flatten()
        return pa.array(array, type=data_type)
