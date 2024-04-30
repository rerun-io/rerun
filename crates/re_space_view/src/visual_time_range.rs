//! For the most part this module bridges older visual history types to the newer
//! visual range types that are defined in the `re_types` crate.
//!
//! Historically there was a special `EntityProperty` bag that was used to store the visual history.
//! Now, visual history makes use of the component override system (components stored at special paths in the blueprint store).
//!
//! The intent is to eventually remove the old types, but this bridge here is there in order
//! to reduce the amount of changes in code that is likely to be refactored soon anyways.

use re_query::VisibleHistoryBoundary;
use re_types::datatypes::{VisibleTimeRangeBoundary, VisibleTimeRangeBoundaryKind};

pub fn visible_history_boundary_from_time_range_boundary(
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
