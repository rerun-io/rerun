from __future__ import annotations

from typing import Any

from ... import datatypes
from ...error_utils import _send_warning_or_raise, catch_and_log_exceptions


class VisibleTimeRangesExt:
    """Extension for [VisibleTimeRanges][rerun.blueprint.archetypes.VisibleTimeRanges]."""

    def __init__(self: Any, ranges: datatypes.VisibleTimeRangeArrayLike):
        """
        Create a new instance of the VisibleTimeRanges archetype.

        Parameters
        ----------
        ranges:
            The time ranges to show for each timeline unless specified otherwise on a per-entity basis.

            If a timeline is listed twice, a warning will be issued and the first entry will be used.

        """

        if isinstance(ranges, datatypes.VisibleTimeRange):
            ranges = [ranges]

        timelines = set()
        for range in ranges:
            if range.timeline in timelines:
                _send_warning_or_raise(
                    f"Warning: Timeline {range.timeline} is listed twice in the list of visible time ranges. Only the first entry will be used.",
                    1,
                )
            timelines.add(range.timeline)

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(ranges=ranges)
            return
        self.__attrs_clear__()
