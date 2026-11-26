from __future__ import annotations

from typing import cast
from unittest.mock import Mock

import pytest
import rerun as rr
import rerun.blueprint as rrb
from rerun.error_utils import RerunWarning


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


@pytest.mark.filterwarnings("error::rerun.error_utils.RerunWarning")
@pytest.mark.parametrize("timelines", [("first",), ("first", "second")])
def test_visible_time_range_entity_override(timelines: tuple[str, ...]) -> None:
    time_ranges = [
        rr.VisibleTimeRange(
            timeline,
            start=rr.TimeRangeBoundary.cursor_relative(),
            end=rr.TimeRangeBoundary.infinite(),
        )
        for timeline in timelines
    ]
    override = time_ranges[0] if len(time_ranges) == 1 else time_ranges
    view = rrb.TimeSeriesView(overrides={"/action": override})
    stream = Mock(spec=rr.RecordingStream)

    view._log_to_stream(stream)

    entity_path = f"{view.blueprint_path()}/ViewContents/overrides/action/visualizers"
    assert [call.args for call in stream.log.call_args_list if call.args[0] == entity_path] == [
        (entity_path, rrb.VisibleTimeRanges(time_ranges)),
    ]


@pytest.mark.parametrize("override_kind", ["range", "ranges", "component_batch"])
def test_visible_time_range_visualizer_override_warns(override_kind: str) -> None:
    time_range = rr.VisibleTimeRange(
        "timeline",
        start=rr.TimeRangeBoundary.cursor_relative(),
        end=rr.TimeRangeBoundary.infinite(),
    )
    time_ranges = rrb.VisibleTimeRanges(time_range)
    invalid_override = {
        "range": time_range,
        "ranges": time_ranges,
        "component_batch": time_ranges.as_component_batches()[0],
    }[override_kind]
    line_color = rr.SeriesLines.from_fields(colors=[[255, 0, 0]]).as_component_batches()
    visualizer = rrb.Visualizer(
        "SeriesLines", overrides=cast("list[rr.DescribedComponentBatch]", [*line_color, invalid_override])
    )
    view = rrb.TimeSeriesView(overrides={"/action": visualizer})
    stream = Mock(spec=rr.RecordingStream)

    previous_strict_mode = rr.strict_mode()
    try:
        rr.set_strict_mode(False)
        with pytest.warns(RerunWarning, match=r"Visible time ranges.*Visualizer.*view's overrides"):
            view._log_to_stream(stream)
    finally:
        rr.set_strict_mode(previous_strict_mode)

    visualizer_path = f"{view.blueprint_path()}/ViewContents/overrides/action/visualizers/{visualizer.id}"
    stream.log.assert_any_call(visualizer_path, line_color)
