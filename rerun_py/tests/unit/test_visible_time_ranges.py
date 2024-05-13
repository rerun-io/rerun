from __future__ import annotations

import pytest
import rerun as rr


def test_visible_time_ranges_warns_on_duplicate_entry() -> None:
    rr.set_strict_mode(True)

    with pytest.raises(ValueError):
        rr.blueprint.archetypes.VisibleTimeRanges([
            rr.VisibleTimeRange("timeline", start=rr.TimeRangeBoundary.infinite(), end=rr.TimeRangeBoundary.infinite()),
            rr.VisibleTimeRange(
                "timeline", start=rr.TimeRangeBoundary.absolute(seconds=1.0), end=rr.TimeRangeBoundary.cursor_relative()
            ),
        ])


def test_visible_time_ranges_from_single() -> None:
    time_range = rr.VisibleTimeRange(
        "timeline", start=rr.TimeRangeBoundary.cursor_relative(), end=rr.TimeRangeBoundary.absolute(seconds=1.0)
    )
    assert rr.blueprint.archetypes.VisibleTimeRanges(time_range) == rr.blueprint.archetypes.VisibleTimeRanges([
        time_range
    ])
