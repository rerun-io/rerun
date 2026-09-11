//! Shared behavior and conversions for views with a `TimeAxis`.
//!
//! Any view that lets the user pan and zoom along time persists that window here, and any two
//! views that link their time axis share the one stored on
//! [`re_viewer_context::GLOBAL_VIEW_ID`] — so they have to agree, to the unit, on how the
//! component maps to a window and back.

use re_log_types::{AbsoluteTimeRange, AbsoluteTimeRangeF, TimeReal, TimeType};
use re_sdk_types::encodings::{TimeInt, TimeRange, TimeRangeBoundary};
use re_viewer_context::{TimeControlCommand, ViewerContext};

/// Resolve a `TimeAxis:view_range` into the absolute pan/zoom window it denotes.
///
/// Infinite boundaries mean "as far as the recording goes" and resolve to `timeline_range`;
/// cursor-relative ones resolve against `cursor`.
///
/// The result is whatever the stored range says, so it can be inverted or empty if that is what
/// the user configured — callers are expected to make it sane for their own axis.
pub fn resolve_time_axis_range(
    view_range: &TimeRange,
    timeline_range: AbsoluteTimeRange,
    cursor: TimeInt,
) -> AbsoluteTimeRange {
    let min = match view_range.start {
        TimeRangeBoundary::Infinite => timeline_range.min.as_i64(),
        _ => view_range.start.start_boundary_time(cursor).0,
    };
    let max = match view_range.end {
        TimeRangeBoundary::Infinite => timeline_range.max.as_i64(),
        _ => view_range.end.end_boundary_time(cursor).0,
    };

    AbsoluteTimeRange::new(min, max)
}

/// The default pan/zoom window for data spanning `data_span`, for views that shouldn't show all of
/// it at once.
///
/// When viewing large recordings (spanning hours), it is VERY important that we only show part of
/// the data by default, for two reasons:
///
/// # Performance
/// If we show all the data, we need to collect and aggregate all the data. This can be VERY slow.
///
/// # Legibility
/// A sufficiently zoomed out view is indistinguishable from noise.
///
/// The returned window is cursor-relative and symmetric, which pins the time cursor to the center
/// of the view. `None` means the span is small enough that showing everything is fine.
pub fn cursor_centered_default_range(time_type: TimeType, data_span: u64) -> Option<TimeRange> {
    const NS_PER_SEC: i64 = 1_000_000_000;

    match time_type {
        TimeType::Sequence => (2_000 < data_span).then(|| TimeRange::from_cursor_plus_minus(1_000)),

        TimeType::TimestampNs | TimeType::DurationNs => ((60 * NS_PER_SEC as u64) < data_span)
            .then(|| TimeRange::from_cursor_plus_minus(30 * NS_PER_SEC)),
    }
}

/// Adjust a `TimeAxis:view_range` so it keeps showing the same stretch of time when the time cursor
/// moves from `from` to `to`.
pub fn time_axis_range_after_cursor_move(
    view_range: &TimeRange,
    from: i64,
    to: i64,
) -> Option<TimeRange> {
    let time_diff = from - to;

    let mut any_relative = false;
    let mut map_boundary = |boundary| {
        if let TimeRangeBoundary::CursorRelative(offset) = boundary {
            any_relative = true;
            TimeRangeBoundary::CursorRelative((offset.0 + time_diff).into())
        } else {
            boundary
        }
    };

    let shifted = TimeRange {
        start: map_boundary(view_range.start),
        end: map_boundary(view_range.end),
    };

    any_relative.then_some(shifted)
}

/// Move the time cursor to `time` and pause, keeping a cursor-relative pan/zoom window in place.
pub fn set_time_cursor(
    ctx: &ViewerContext<'_>,
    current_time: Option<i64>,
    view_range: Option<&TimeRange>,
    time: TimeReal,
) -> Option<TimeRange> {
    // Adjust the view range against the time the command will actually set. Otherwise dragging
    // outside the recording would shift the range farther than the clamped cursor moves.
    let time = ctx
        .recording()
        .time_range_for(ctx.time_ctrl.timeline_name())
        .map_or(time, |timeline_range| {
            let timeline_range = AbsoluteTimeRangeF::from(timeline_range);
            time.clamp(timeline_range.min, timeline_range.max)
        });

    let new_view_range = current_time.and_then(|current_time| {
        let view_range = view_range?;
        time_axis_range_after_cursor_move(view_range, current_time, time.floor().as_i64())
    });

    // Set the cursor before pausing so `last_paused_time` records its new position.
    ctx.send_time_commands([
        TimeControlCommand::SetTimeClamped(time),
        TimeControlCommand::Pause,
    ]);

    new_view_range
}

/// Convert a pan/zoom boundary from plot space to an absolute timeline time.
///
/// Both window boundaries round to the nearest integer, with ties away from zero in plot space,
/// before adding `time_offset` with saturation. Pass `0` for windows already in timeline units.
pub fn time_axis_time_from_plot(value: TimeReal, time_offset: i64) -> TimeInt {
    let rounded: TimeInt = value.round().into();
    TimeInt(rounded.0.saturating_add(time_offset))
}

/// Preserve each cursor-relative or infinite boundary after zooming or panning.
///
/// `min` and `max` are absolute timeline times. Cursor-relative boundaries become absolute if
/// their new offset cannot be represented.
pub fn recover_relative_boundaries_after_zoom_or_pan(
    min: TimeInt,
    max: TimeInt,
    previous: &TimeRange,
    cursor: TimeInt,
) -> TimeRange {
    let boundary = |value: TimeInt, previous| match previous {
        TimeRangeBoundary::Infinite => TimeRangeBoundary::Infinite,
        TimeRangeBoundary::CursorRelative(_) => value
            .0
            .checked_sub(cursor.0)
            .map_or(TimeRangeBoundary::Absolute(value), |offset| {
                TimeRangeBoundary::CursorRelative(TimeInt(offset))
            }),
        TimeRangeBoundary::Absolute(_) => TimeRangeBoundary::Absolute(value),
    };

    TimeRange {
        start: boundary(min, previous.start),
        end: boundary(max, previous.end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of shifting the boundaries: the window the range denotes doesn't move.
    #[test]
    fn test_time_axis_range_after_cursor_move_keeps_window() {
        let timeline_range = AbsoluteTimeRange::new(0, 10_000);
        let cursor = 4_000;

        for view_range in [
            TimeRange::from_cursor_plus_minus(1_000),
            TimeRange {
                start: TimeRangeBoundary::CursorRelative(TimeInt(-100)),
                end: TimeRangeBoundary::Absolute(TimeInt(5_000)),
            },
        ] {
            let window = resolve_time_axis_range(&view_range, timeline_range, TimeInt(cursor));

            for new_cursor in [cursor - 2_500, cursor + 2_500] {
                let shifted = time_axis_range_after_cursor_move(&view_range, cursor, new_cursor)
                    .expect("cursor-relative range should be shifted");

                assert_eq!(
                    resolve_time_axis_range(&shifted, timeline_range, TimeInt(new_cursor)),
                    window
                );
            }
        }
    }

    /// An absolute range doesn't follow the cursor in the first place, so there is nothing to shift.
    #[test]
    fn test_time_axis_range_after_cursor_move_leaves_absolute_range_alone() {
        let absolute = TimeRange {
            start: TimeRangeBoundary::Absolute(TimeInt(0)),
            end: TimeRangeBoundary::Absolute(TimeInt(100)),
        };

        assert_eq!(
            time_axis_range_after_cursor_move(&absolute, 10, 20),
            None,
            "absolute range"
        );
        assert_eq!(
            time_axis_range_after_cursor_move(&TimeRange::EVERYTHING, 10, 20),
            None,
            "infinite range"
        );
    }
}
