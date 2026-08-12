//! Conversions between a view's pan/zoom window and the `TimeAxis:view_range` blueprint component.
//!
//! Any view that lets the user pan and zoom along time persists that window here, and any two
//! views that link their time axis share the one stored on
//! [`re_viewer_context::GLOBAL_VIEW_ID`] — so they have to agree, to the unit, on how the
//! component maps to a window and back.

use re_log_types::{AbsoluteTimeRange, TimeReal};
use re_sdk_types::datatypes::{TimeInt, TimeRange, TimeRangeBoundary};

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

/// Build a `TimeAxis:view_range` for a pan/zoom time window.
///
/// `min`/`max` are in plot space, rounded down and up respectively, i.e. offset by `time_offset`. Pass `0`
/// if the window is in timeline units already.
pub fn time_axis_range_from_window(min: TimeReal, max: TimeReal, time_offset: i64) -> TimeRange {
    let boundary = |value: TimeReal| {
        TimeRangeBoundary::Absolute(TimeInt(value.round().as_i64().saturating_add(time_offset)))
    };

    TimeRange {
        start: boundary(min),
        end: boundary(max),
    }
}
