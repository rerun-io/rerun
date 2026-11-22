from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
import rerun.blueprint as rrb
from rerun.error_utils import RerunWarning

if TYPE_CHECKING:
    from pathlib import Path


def test_visible_time_ranges_warns_on_duplicate_entry() -> None:
    rr.set_strict_mode(True)

    with pytest.raises(ValueError):
        rr.blueprint.archetypes.VisibleTimeRanges([
            rr.VisibleTimeRange("timeline", start=rr.TimeRangeBoundary.infinite(), end=rr.TimeRangeBoundary.infinite()),
            rr.VisibleTimeRange(
                "timeline",
                start=rr.TimeRangeBoundary.absolute(seconds=1.0),
                end=rr.TimeRangeBoundary.cursor_relative(),
            ),
        ])


def test_visible_time_ranges_from_single() -> None:
    time_range = rr.VisibleTimeRange(
        "timeline",
        start=rr.TimeRangeBoundary.cursor_relative(),
        end=rr.TimeRangeBoundary.absolute(seconds=1.0),
    )
    assert rr.blueprint.archetypes.VisibleTimeRanges(time_range) == rr.blueprint.archetypes.VisibleTimeRanges([
        time_range,
    ])

    assert rr.blueprint.archetypes.VisibleTimeRanges(time_range) == rr.blueprint.archetypes.VisibleTimeRanges(
        timeline="timeline",
        start=rr.TimeRangeBoundary.cursor_relative(),
        end=rr.TimeRangeBoundary.absolute(seconds=1.0),
    )

    assert rr.blueprint.archetypes.VisibleTimeRanges(time_range) == rr.blueprint.archetypes.VisibleTimeRanges(
        timeline="timeline",
        range=rr.TimeRange(rr.TimeRangeBoundary.cursor_relative(), rr.TimeRangeBoundary.absolute(seconds=1.0)),
    )


def test_visible_time_ranges_invalid_parameters() -> None:
    time_range = rr.VisibleTimeRange(
        "timeline",
        start=rr.TimeRangeBoundary.cursor_relative(),
        end=rr.TimeRangeBoundary.absolute(seconds=1.0),
    )

    with pytest.raises(ValueError):
        # Numpy correctly flags this as an invalid overload, make sure it also throws.
        rr.blueprint.archetypes.VisibleTimeRanges(  # ty: ignore[no-matching-overload]
            ranges=[time_range],
            timeline="timeline",
            start=rr.TimeRangeBoundary.cursor_relative(),
            end=rr.TimeRangeBoundary.absolute(seconds=1.0),
        )  # type: ignore[call-overload]

    with pytest.raises(ValueError):
        # Numpy correctly flags this as an invalid overload, make sure it also throws.
        rr.blueprint.archetypes.VisibleTimeRanges(  # ty: ignore[no-matching-overload]
            timeline="timeline",
            start=rr.TimeRangeBoundary.cursor_relative(),
            end=rr.TimeRangeBoundary.absolute(seconds=1.0),
            range=rr.TimeRange(rr.TimeRangeBoundary.cursor_relative(), rr.TimeRangeBoundary.absolute(seconds=1.0)),
        )  # type: ignore[call-overload]


def test_visible_time_range_override_warns(tmp_path: Path) -> None:
    time_ranges = [
        rr.VisibleTimeRange(
            timeline,
            start=rr.TimeRangeBoundary.cursor_relative(),
            end=rr.TimeRangeBoundary.infinite(),
        )
        for timeline in ("first", "second")
    ]
    blueprint = rrb.Blueprint(
        rrb.TimeSeriesView(overrides={"/action": time_ranges})  # type: ignore[dict-item]
    )

    previous_strict_mode = rr.strict_mode()
    try:
        rr.set_strict_mode(False)
        with pytest.warns(RerunWarning, match=r"overrides.*all ranges for each entity in a single VisibleTimeRanges"):
            blueprint.save("test_visible_time_range_override", tmp_path / "bad.rbl")
    finally:
        rr.set_strict_mode(previous_strict_mode)
