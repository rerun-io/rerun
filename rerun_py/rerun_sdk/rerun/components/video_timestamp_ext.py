from __future__ import annotations

from typing import Any, Union

import numpy as np
import numpy.typing as npt

from rerun.error_utils import catch_and_log_exceptions

from .. import components, datatypes


class VideoTimestampExt:
    """Extension for [VideoTimestamp][rerun.components.VideoTimestamp]."""

    def __init__(
        self: Any,
        *,
        video_time: Union[int, None] = None,
        time_mode: Union[datatypes.VideoTimeModeLike, None] = None,
        seconds: Union[float, None] = None,
    ):
        """
        Create a new instance of the VideoTimestamp component.

        Parameters
        ----------
        video_time:
            Timestamp value, type defined by `time_mode`.
        time_mode:
            How to interpret `video_time`.
        seconds:
            The timestamp in seconds since the start of the video.
            Mutually exclusive with `video_time` and `time_mode`.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if seconds is not None:
                if video_time is not None or time_mode is not None:
                    raise ValueError("Cannot specify both `seconds` and `video_time`/`time_mode`.")
                video_time = int(seconds * 1e9)
                time_mode = datatypes.VideoTimeMode.Nanoseconds

            self.__attrs_init__(video_time=video_time, time_mode=time_mode)
            return

        self.__attrs_clear__()

    @staticmethod
    def seconds(
        seconds: npt.ArrayLike,
    ) -> components.VideoTimestampBatch:
        """
        Create a video timestamp batch from seconds since video start.

        Parameters
        ----------
        seconds:
            Timestamp values in seconds since video start.

        """
        return components.VideoTimestamp.nanoseconds(np.array(seconds) * 1e9)

    @staticmethod
    def milliseconds(
        milliseconds: npt.ArrayLike,
    ) -> components.VideoTimestampBatch:
        """
        Create a video timestamp batch from milliseconds since video start.

        Parameters
        ----------
        milliseconds:
            Timestamp values in milliseconds since video start.

        """
        return components.VideoTimestamp.nanoseconds(np.array(milliseconds) * 1e6)

    @staticmethod
    def nanoseconds(
        nanoseconds: npt.ArrayLike,
    ) -> components.VideoTimestampBatch:
        """
        Create a video timestamp batch from nanoseconds since video start.

        Parameters
        ----------
        nanoseconds:
            Timestamp values in nanoseconds since video start.

        """
        nanoseconds = np.asarray(nanoseconds, dtype=np.int64)

        return components.VideoTimestampBatch([
            components.VideoTimestamp(video_time=ns, time_mode=datatypes.VideoTimeMode.Nanoseconds)
            for ns in nanoseconds
        ])
