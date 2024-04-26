use super::{VisibleTimeRange, VisibleTimeRangeBoundary};

impl VisibleTimeRange {
    pub const EMPTY: Self = Self {
        start: VisibleTimeRangeBoundary::AT_CURSOR,
        end: VisibleTimeRangeBoundary::AT_CURSOR,
    };

    pub const EVERYTHING: Self = Self {
        start: VisibleTimeRangeBoundary::INFINITE,
        end: VisibleTimeRangeBoundary::INFINITE,
    };
}
