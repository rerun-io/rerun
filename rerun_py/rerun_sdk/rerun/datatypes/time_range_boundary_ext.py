from __future__ import annotations

from typing import TYPE_CHECKING, Any

from .. import datatypes

if TYPE_CHECKING:
    from .time_range_boundary import TimeRangeBoundary


class TimeRangeBoundaryExt:
    """Extension for [TimeRangeBoundary][rerun.datatypes.TimeRangeBoundary]."""

    @staticmethod
    def cursor_relative(time: datatypes.TimeIntLike = 0) -> TimeRangeBoundary:
        from .time_range_boundary import TimeRangeBoundary

        return TimeRangeBoundary(inner=time, kind="cursor_relative")

    @staticmethod
    def infinite(time: datatypes.TimeIntLike = 0) -> TimeRangeBoundary:
        from .time_range_boundary import TimeRangeBoundary

        return TimeRangeBoundary(inner=None, kind="infinite")

    @staticmethod
    def absolute(time: datatypes.TimeIntLike) -> TimeRangeBoundary:
        from .time_range_boundary import TimeRangeBoundary

        return TimeRangeBoundary(inner=time, kind="absolute")
