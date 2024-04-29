use super::{VisibleTimeRange, VisibleTimeRangeBoundary};

impl VisibleTimeRange {
    /// The empty range, set at the current time cursor.
    pub const EMPTY: Self = Self {
        start: VisibleTimeRangeBoundary::AT_CURSOR,
        end: VisibleTimeRangeBoundary::AT_CURSOR,
    };

    /// The range encompassing all time, from -∞ to +∞.
    pub const EVERYTHING: Self = Self {
        // This means -∞
        start: VisibleTimeRangeBoundary::INFINITE,

        // This means +∞
        end: VisibleTimeRangeBoundary::INFINITE,
    };
}

impl Default for VisibleTimeRange {
    fn default() -> Self {
        Self::EMPTY
    }
}
