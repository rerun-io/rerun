//! This module bridges older visual history types to the newer visual range types that are defined
//! in the `re_types` crate.
//!
//! Historically there was a special `EntityProperty` bag that was used to store the visual history.
//! Now, visual history makes use of the component override system (components stored at special paths in the blueprint store).
//!
//! The intent is to eventually remove the old types, but this bridge here is there in order
//! to reduce the amount of changes in code that is likely to be refactored soon anyways.

use re_query::{ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary};
use re_types::blueprint::{
    components::VisibleTimeRange,
    datatypes::{VisibleTimeRangeBoundary, VisibleTimeRangeBoundaryKind},
};
use re_viewer_context::{SpaceViewClassIdentifier, ViewerContext};

fn time_range_boundary_to_visual_history_boundary(
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

fn visible_history_from_boundaries(
    from: &VisibleTimeRangeBoundary,
    to: &VisibleTimeRangeBoundary,
) -> VisibleHistory {
    VisibleHistory {
        from: time_range_boundary_to_visual_history_boundary(from),
        to: time_range_boundary_to_visual_history_boundary(to),
    }
}

pub fn query_visual_history(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
) -> ExtraQueryHistory {
    let visual_time_range_component =
        data_result.lookup_override::<re_types::blueprint::components::VisibleTimeRange>(ctx);
    if let Some(visual_time_range_component) = visual_time_range_component {
        ExtraQueryHistory {
            enabled: true,
            nanos: visible_history_from_boundaries(
                &visual_time_range_component.0.from_time,
                &visual_time_range_component.0.to_time,
            ),
            sequences: visible_history_from_boundaries(
                &visual_time_range_component.0.from_sequence,
                &visual_time_range_component.0.to_sequence,
            ),
        }
    } else {
        ExtraQueryHistory {
            enabled: false,
            nanos: VisibleHistory::default(),
            sequences: VisibleHistory::default(),
        }
    }
}

// TODO(#4194): this should come from delegation to the space-view-class
pub fn default_time_range(class_identifier: SpaceViewClassIdentifier) -> VisibleTimeRange {
    if class_identifier == "Time Series" {
        VisibleTimeRange::EVERYTHING.clone()
    } else {
        VisibleTimeRange::EMPTY.clone()
    }
}
