from __future__ import annotations

from typing import Any

from .. import datatypes


class VisibleTimeRangeExt:
    """Extension for [VisibleTimeRange][rerun.datatypes.VisibleTimeRange]."""

    def __init__(
        self: Any,
        timeline: datatypes.Utf8Like,
        range: datatypes.TimeRangeLike | None = None,
        *,
        start: datatypes.TimeRangeBoundary | None = None,
        end: datatypes.TimeRangeBoundary | None = None,
    ):
        """
        Create a new instance of the VisibleTimeRange datatype.

        Parameters
        ----------
        timeline:
            Name of the timeline this applies to.
        range:
            Time range to use for this timeline.
        start:
            Low time boundary for sequence timeline. Specify this instead of `range`.
        end:
            High time boundary for sequence timeline. Specify this instead of `range`.

        """
        from . import TimeRange

        if range is None:
            if start is None or end is None:
                raise ValueError("Specify either start_and_end or both start & end")
            range = TimeRange(start=start, end=end)
        else:
            if start is not None or end is not None:
                raise ValueError("Specify either start_and_end or both start & end, not both")

        self.__attrs_init__(timeline=timeline, range=range)
