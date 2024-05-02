use super::{VisibleTimeRange, VisibleTimeRangeBoundary};

impl VisibleTimeRange {
    /// The range encompassing all time, from -∞ to +∞.
    pub const EVERYTHING: Self = Self {
        // This means -∞
        start: VisibleTimeRangeBoundary::INFINITE,

        // This means +∞
        end: VisibleTimeRangeBoundary::INFINITE,
    };

    /// A range of zero length exactly at the time cursor.
    ///
    /// This is *not* the same as latest-at queries and queries the state that was logged exactly at the cursor.
    /// In contrast, latest-at queries each component's latest known state.
    pub const AT_CURSOR: Self = Self {
        start: VisibleTimeRangeBoundary::AT_CURSOR,
        end: VisibleTimeRangeBoundary::AT_CURSOR,
    };
}
