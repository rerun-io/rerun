from __future__ import annotations

from typing import Any

from .. import components, datatypes
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions


class VideoFrameReferenceExt:
    """Extension for [VideoFrameReference][rerun.archetypes.VideoFrameReference]."""

    def __init__(
        self: Any,
        timestamp: datatypes.VideoTimestampLike | None = None,
        *,
        seconds: float | None = None,
        nanoseconds: int | None = None,
        video_reference: datatypes.EntityPathLike | None = None,
    ) -> None:
        """
        Create a new instance of the VideoFrameReference archetype.

        Parameters
        ----------
        timestamp:
            References the closest video frame to this timestamp.

            Note that this uses the closest video frame instead of the latest at this timestamp
            in order to be more forgiving of rounding errors for inprecise timestamp types.

            Mutally exclusive with `seconds` and `nanoseconds`.
        seconds:
            Sets the timestamp to the given number of seconds.

            Mutally exclusive with `timestamp` and `nanoseconds`.
        nanoseconds:
            Sets the timestamp to the given number of nanoseconds.

            Mutally exclusive with `timestamp` and `seconds`.
        video_reference:
            Optional reference to an entity with a [`archetypes.AssetVideo`][rerun.archetypes.AssetVideo].

            If none is specified, the video is assumed to be at the same entity.
            Note that blueprint overrides on the referenced video will be ignored regardless,
            as this is always interpreted as a reference to the data store.

            For a series of video frame references, it is recommended to specify this path only once
            at the beginning of the series and then rely on latest-at query semantics to
            keep the video reference active.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if timestamp is None:
                if seconds is None and nanoseconds is None:
                    raise ValueError("Either timestamp or seconds/nanoseconds must be specified.")
                timestamp = components.VideoTimestamp(seconds=seconds, nanoseconds=nanoseconds)
            elif seconds is not None or nanoseconds is not None:
                raise ValueError("Cannot specify both `timestamp` and `seconds`/`nanoseconds`.")
            elif isinstance(timestamp, float):
                _send_warning_or_raise("Timestamp can't be specified as a float, use `seconds` instead.")

            self.__attrs_init__(
                timestamp=timestamp,
                video_reference=video_reference,
            )
            return

        self.__attrs_clear__()
