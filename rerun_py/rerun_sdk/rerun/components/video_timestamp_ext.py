from __future__ import annotations

from typing import Any, Union

import numpy as np
import numpy.typing as npt

from rerun.error_utils import catch_and_log_exceptions

from .. import components


class VideoTimestampExt:
    """Extension for [VideoTimestamp][rerun.components.VideoTimestamp]."""

    def __init__(
        self: Any,
        *,
        nanoseconds: Union[int, None] = None,
        seconds: Union[float, None] = None,
    ):
        """
        Create a new instance of the VideoTimestamp component.

        Parameters
        ----------
        nanoseconds:
            Presentation timestamp in nanoseconds.
            Mutually exclusive with `seconds`.
        seconds:
            Presentation timestamp in seconds.
            Mutually exclusive with `nanoseconds`.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if seconds is not None:
                if nanoseconds is not None:
                    raise ValueError("Cannot specify both `seconds` and `nanoseconds`.")
                nanoseconds = int(seconds * 1e9 + 0.5)

            self.__attrs_init__(timestamp_ns=nanoseconds)
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

        return components.VideoTimestampBatch([components.VideoTimestamp(nanoseconds=ns) for ns in nanoseconds])
