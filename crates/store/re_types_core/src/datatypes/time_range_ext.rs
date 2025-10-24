use super::{TimeRange, TimeRangeBoundary};

impl TimeRange {
    /// The range encompassing all time, from beginning to end.
    pub const EVERYTHING: Self = Self {
        // This means the beginning.
        start: TimeRangeBoundary::Infinite,

        // This means the end.
        end: TimeRangeBoundary::Infinite,
    };

    /// A range of zero length exactly at the time cursor.
    ///
    /// This is *not* the same as latest-at queries and queries the state that was logged exactly at the cursor.
    /// In contrast, latest-at queries each component's latest known state.
    pub const AT_CURSOR: Self = Self {
        start: TimeRangeBoundary::AT_CURSOR,
        end: TimeRangeBoundary::AT_CURSOR,
    };
}
