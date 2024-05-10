from __future__ import annotations

from typing import TYPE_CHECKING

from . import TimeInt, TimeIntLike

if TYPE_CHECKING:
    from .time_range_boundary import TimeRangeBoundary


class TimeRangeBoundaryExt:
    """Extension for [TimeRangeBoundary][rerun.datatypes.TimeRangeBoundary]."""

    @staticmethod
    def cursor_relative(time: TimeIntLike = 0) -> TimeRangeBoundary:
        """
        Boundary that is relative to the timeline cursor.

        Parameters
        ----------
        time:
            Offset from the cursor time, can be positive or negative.

        """

        from .time_range_boundary import TimeRangeBoundary

        if not isinstance(time, TimeInt):
            time = TimeInt(time)

        return TimeRangeBoundary(inner=time, kind="cursor_relative")

    @staticmethod
    def infinite() -> TimeRangeBoundary:
        """
        Boundary that extends to infinity.

        Depending on the context, this can mean the beginning or the end of the timeline.
        """

        from .time_range_boundary import TimeRangeBoundary

        return TimeRangeBoundary(inner=None, kind="infinite")

    @staticmethod
    def absolute(time: TimeIntLike) -> TimeRangeBoundary:
        """
        Boundary that is at an absolute time.

        Parameters
        ----------
        time:
            Absolute time value.

        """

        from .time_range_boundary import TimeRangeBoundary

        if not isinstance(time, TimeInt):
            time = TimeInt(time)

        return TimeRangeBoundary(inner=time, kind="absolute")
