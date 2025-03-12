from __future__ import annotations

from typing import Any, overload

from rerun.datatypes.visible_time_range import VisibleTimeRange

from ... import datatypes
from ...error_utils import _send_warning_or_raise, catch_and_log_exceptions


class VisibleTimeRangesExt:
    """Extension for [VisibleTimeRanges][rerun.blueprint.archetypes.VisibleTimeRanges]."""

    @overload
    def __init__(self: Any, ranges: datatypes.VisibleTimeRangeArrayLike) -> None: ...

    @overload
    def __init__(
        self: Any,
        *,
        timeline: datatypes.Utf8Like,
        range: datatypes.TimeRangeLike,
    ) -> None: ...

    @overload
    def __init__(
        self: Any,
        *,
        timeline: datatypes.Utf8Like,
        start: datatypes.TimeRangeBoundary,
        end: datatypes.TimeRangeBoundary,
    ) -> None: ...

    def __init__(
        self: Any,
        ranges: datatypes.VisibleTimeRangeArrayLike | None = None,
        *,
        timeline: datatypes.Utf8Like | None = None,
        range: datatypes.TimeRangeLike | None = None,
        start: datatypes.TimeRangeBoundary | None = None,
        end: datatypes.TimeRangeBoundary | None = None,
    ) -> None:
        """
        Create a new instance of the VisibleTimeRanges archetype.

        Either from a list of `VisibleTimeRange` objects, or from a single `timeline`-name plus either `range` or `start` & `end`.

        Parameters
        ----------
        ranges:
            The time ranges to show for each timeline unless specified otherwise on a per-entity basis.

            If a timeline is listed twice, a warning will be issued and the first entry will be used.

        timeline:
            The name of the timeline to show.
            Mutually exclusive with `ranges`.
        range:
            The range of the timeline to show.
            Requires `timeline` to be set. Mutually exclusive with `start` & `end`.
        start:
            The start of the timeline to show.
            Requires `timeline` to be set. Mutually exclusive with `range`.
        end:
            The end of the timeline to show.
            Requires `timeline` to be set. Mutually exclusive with `range`.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if timeline is not None:
                if ranges is not None:
                    raise ValueError("`timeline` and `ranges` are mutually exclusive.")
                ranges = [VisibleTimeRange(timeline=timeline, range=range, start=start, end=end)]
            elif ranges is None:
                raise ValueError("Either `ranges` or `timeline` must be set.")

            if isinstance(ranges, datatypes.VisibleTimeRange):
                ranges = [ranges]

            timelines = set()
            for visible_time_range in ranges:
                if visible_time_range.timeline in timelines:
                    _send_warning_or_raise(
                        f"Warning: Timeline {visible_time_range.timeline} is listed twice in the list of visible time ranges. Only the first entry will be used.",
                        1,
                    )
                timelines.add(visible_time_range.timeline)

            self.__attrs_init__(ranges=ranges)
            return

        self.__attrs_clear__()
