from __future__ import annotations

from typing import TYPE_CHECKING

from . import TimeInt

if TYPE_CHECKING:
    from .time_range_boundary import TimeRangeBoundary


class TimeRangeBoundaryExt:
    """Extension for [TimeRangeBoundary][rerun.datatypes.TimeRangeBoundary]."""

    @staticmethod
    def cursor_relative(
        offset: TimeInt | None = None, *, seq: int | None = None, seconds: float | None = None, nanos: int | None = None
    ) -> TimeRangeBoundary:
        """
        Boundary that is relative to the timeline cursor.

        The offset can be positive or negative.
        An offset of zero (the default) means the cursor time itself.

        Parameters
        ----------
        offset:
            Offset from the cursor time.

            Mutually exclusive with seq, seconds and nanos.
        seq:
            Offset in sequence numbers.

            Use this for sequence timelines.
            Mutually exclusive with time, seconds and nanos.
        seconds:
            Offset in seconds.

            Use this for time based timelines.
            Mutually exclusive with time, seq and nanos.
        nanos:
            Offset in nanoseconds.

            Use this for time based timelines.
            Mutually exclusive with time, seq and seconds.

        """

        from .time_range_boundary import TimeRangeBoundary

        if offset is None:
            if seq is None and seconds is None and nanos is None:
                offset = TimeInt(seq=0)
            else:
                offset = TimeInt(seq=seq, seconds=seconds, nanos=nanos)
        elif seq is not None or seconds is not None or nanos is not None:
            raise ValueError("Only one of time, seq, seconds, or nanos can be provided.")

        return TimeRangeBoundary(inner=offset, kind="cursor_relative")

    @staticmethod
    def infinite() -> TimeRangeBoundary:
        """
        Boundary that extends to infinity.

        Depending on the context, this can mean the beginning or the end of the timeline.
        """

        from .time_range_boundary import TimeRangeBoundary

        return TimeRangeBoundary(inner=None, kind="infinite")

    @staticmethod
    def absolute(
        time: TimeInt | None = None, *, seq: int | None = None, seconds: float | None = None, nanos: int | None = None
    ) -> TimeRangeBoundary:
        """
        Boundary that is at an absolute time.

        Parameters
        ----------
        time:
            Absolute time.

            Mutually exclusive with seq, seconds and nanos.
        seq:
            Absolute time in sequence numbers.

            Use this for sequence timelines.
            Mutually exclusive with time, seconds and nanos.
        seconds:
            Absolute time in seconds.

            Use this for time based timelines.
            Mutually exclusive with time, seq and nanos.
        nanos:
            Absolute time in nanoseconds.

            Use this for time based timelines.
            Mutually exclusive with time, seq and seconds.

        """

        from .time_range_boundary import TimeRangeBoundary

        if time is None:
            time = TimeInt(seq=seq, seconds=seconds, nanos=nanos)
        elif seq is not None or seconds is not None or nanos is not None:
            raise ValueError("Only one of time, seq, seconds, or nanos can be provided.")

        return TimeRangeBoundary(inner=time, kind="absolute")
