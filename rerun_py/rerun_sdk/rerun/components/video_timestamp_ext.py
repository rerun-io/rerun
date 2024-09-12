from __future__ import annotations

import numpy as np
import numpy.typing as npt

from .. import components, datatypes


class VideoTimestampExt:
    """Extension for [VideoTimestamp][rerun.components.VideoTimestamp]."""

    # Implementation note:
    # We could add an init method that deals with seconds/milliseconds/nanoseconds etc.
    # However, this would require _a lot_ of slow parameter validation on a per timestamp basis.
    # When in actuallity, this data practically always comes in homogeneous batches.

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
            Timestamp values in milliseconds since video start.

        """
        nanoseconds = np.asarray(nanoseconds, dtype=np.int64)

        return components.VideoTimestampBatch([
            components.VideoTimestamp(video_time=ns, time_mode=datatypes.VideoTimeMode.Nanoseconds)
            for ns in nanoseconds
        ])
