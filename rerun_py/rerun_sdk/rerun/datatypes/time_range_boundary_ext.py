from __future__ import annotations

from typing import TYPE_CHECKING, overload

from . import TimeInt

if TYPE_CHECKING:
    from .time_range_boundary import TimeRangeBoundary


class TimeRangeBoundaryExt:
    """Extension for [TimeRangeBoundary][rerun.datatypes.TimeRangeBoundary]."""

    @overload
    @staticmethod
    def cursor_relative() -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def cursor_relative(offset: TimeInt) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def cursor_relative(*, seq: int) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def cursor_relative(*, seconds: float) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def cursor_relative(*, nanos: int) -> TimeRangeBoundary: ...

    @staticmethod
    def cursor_relative(
        offset: TimeInt | None = None,
        *,
        seq: int | None = None,
        seconds: float | None = None,
        nanos: int | None = None,
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
                offset = TimeInt(seq=seq, seconds=seconds, nanos=nanos)  # type: ignore[call-overload]
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

    @overload
    @staticmethod
    def absolute(time: TimeInt) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def absolute(*, seq: int) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def absolute(*, seconds: float) -> TimeRangeBoundary: ...

    @overload
    @staticmethod
    def absolute(*, nanos: int) -> TimeRangeBoundary: ...

    @staticmethod
    def absolute(
        time: TimeInt | None = None,
        *,
        seq: int | None = None,
        seconds: float | None = None,
        nanos: int | None = None,
    ) -> TimeRangeBoundary:
        """
        Boundary that is at an absolute time.

        Exactly one of 'time', 'seq', 'seconds', or 'nanos' must be provided.

        Parameters
        ----------
        time:
            Absolute time.

        seq:
            Absolute time in sequence numbers.

            Not compatible with temporal timelines.

        seconds:
            Absolute time in seconds.

            Interpreted either as a duration or time since unix epoch (depending on timeline type).
            Not compatible with sequence timelines.

        nanos:
            Absolute time in nanoseconds.

            Interpreted either as a duration or time since unix epoch (depending on timeline type).
            Not compatible with sequence timelines.

        """

        if sum(x is not None for x in (time, seq, seconds, nanos)) != 1:
            raise ValueError("Exactly one of 'time', 'seq', 'seconds', or 'nanos' must be provided.")

        from .time_range_boundary import TimeRangeBoundary

        if time is None:
            time = TimeInt(seq=seq, seconds=seconds, nanos=nanos)  # type: ignore[call-overload]

        return TimeRangeBoundary(inner=time, kind="absolute")
