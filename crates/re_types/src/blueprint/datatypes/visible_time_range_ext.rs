use super::{VisibleTimeRange, VisibleTimeRangeBoundary};

impl Default for VisibleTimeRange {
    fn default() -> Self {
        VisibleTimeRange::EMPTY
    }
}

impl VisibleTimeRange {
    pub const EMPTY: Self = Self {
        from_sequence: VisibleTimeRangeBoundary::AT_CURSOR,
        to_sequence: VisibleTimeRangeBoundary::AT_CURSOR,
        from_time: VisibleTimeRangeBoundary::AT_CURSOR,
        to_time: VisibleTimeRangeBoundary::AT_CURSOR,
    };

    pub const EVERYTHING: Self = Self {
        from_sequence: VisibleTimeRangeBoundary::INFINITE,
        to_sequence: VisibleTimeRangeBoundary::INFINITE,
        from_time: VisibleTimeRangeBoundary::INFINITE,
        to_time: VisibleTimeRangeBoundary::INFINITE,
    };
}
