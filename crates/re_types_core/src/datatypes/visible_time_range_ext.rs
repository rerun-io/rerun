use super::{VisibleTimeRange, VisibleTimeRangeBoundary};

impl VisibleTimeRange {
    /// The range encompassing all time, from -∞ to +∞.
    pub const EVERYTHING: Self = Self {
        // This means -∞
        start: VisibleTimeRangeBoundary::INFINITE,

        // This means +∞
        end: VisibleTimeRangeBoundary::INFINITE,
    };
}
