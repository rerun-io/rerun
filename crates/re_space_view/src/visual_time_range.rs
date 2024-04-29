//! For the most part this module bridges older visual history types to the newer
//! visual range types that are defined in the `re_types` crate.
//!
//! Historically there was a special `EntityProperty` bag that was used to store the visual history.
//! Now, visual history makes use of the component override system (components stored at special paths in the blueprint store).
//!
//! The intent is to eventually remove the old types, but this bridge here is there in order
//! to reduce the amount of changes in code that is likely to be refactored soon anyways.

use re_log_types::TimeRange;
use re_query::{ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary};
use re_types::blueprint::datatypes::{
    VisibleTimeRange, VisibleTimeRangeBoundary, VisibleTimeRangeBoundaryKind,
};
use re_viewer_context::ViewerContext;

pub fn time_range_boundary_to_visible_history_boundary(
    boundary: &VisibleTimeRangeBoundary,
) -> VisibleHistoryBoundary {
    match boundary.kind {
        VisibleTimeRangeBoundaryKind::RelativeToTimeCursor => {
            VisibleHistoryBoundary::RelativeToTimeCursor(boundary.time.0)
        }
        VisibleTimeRangeBoundaryKind::Absolute => VisibleHistoryBoundary::Absolute(boundary.time.0),
        VisibleTimeRangeBoundaryKind::Infinite => VisibleHistoryBoundary::Infinite,
    }
}

pub fn visible_history_boundary_to_time_range_boundary(
    boundary: &VisibleHistoryBoundary,
) -> VisibleTimeRangeBoundary {
    match boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(v) => VisibleTimeRangeBoundary {
            kind: VisibleTimeRangeBoundaryKind::RelativeToTimeCursor,
            time: (*v).into(),
        },
        VisibleHistoryBoundary::Absolute(v) => VisibleTimeRangeBoundary {
            kind: VisibleTimeRangeBoundaryKind::Absolute,
            time: (*v).into(),
        },
        VisibleHistoryBoundary::Infinite => VisibleTimeRangeBoundary {
            kind: VisibleTimeRangeBoundaryKind::Infinite,
            time: 0.into(),
        },
    }
}

pub fn visible_time_range_to_time_range(
    range: &VisibleTimeRange,
    cursor: re_log_types::TimeInt,
) -> re_log_types::TimeRange {
    let cursor = cursor.as_i64().into();

    let mut min = range.start.start_boundary_time(cursor);
    let mut max = range.end.end_boundary_time(cursor);

    if min > max {
        std::mem::swap(&mut min, &mut max);
    }

    let min: re_log_types::TimeInt = min.into();
    let max: re_log_types::TimeInt = max.into();

    TimeRange::new(min, max)
}

pub fn query_visual_history(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
) -> ExtraQueryHistory {
    let time_range =
        data_result.lookup_override::<re_types::blueprint::components::VisibleTimeRangeTime>(ctx);
    let sequence_range = data_result
        .lookup_override::<re_types::blueprint::components::VisibleTimeRangeSequence>(ctx);

    let mut history = ExtraQueryHistory {
        enabled: false,
        nanos: Default::default(),
        sequences: Default::default(),
    };

    if let Some(time_range) = time_range {
        history.enabled = true;
        history.nanos = VisibleHistory {
            from: time_range_boundary_to_visible_history_boundary(&time_range.0.start),
            to: time_range_boundary_to_visible_history_boundary(&time_range.0.end),
        };
    }
    if let Some(sequence_range) = sequence_range {
        history.enabled = true;
        history.sequences = VisibleHistory {
            from: time_range_boundary_to_visible_history_boundary(&sequence_range.0.start),
            to: time_range_boundary_to_visible_history_boundary(&sequence_range.0.end),
        };
    }

    history
}
