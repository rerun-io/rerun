use super::{TimeInt, TimeRangeBoundary};

impl TimeRangeBoundary {
    /// Put the boundary at the current time cursor.
    pub const AT_CURSOR: Self = Self::CursorRelative(TimeInt(0));

    /// Returns the time assuming this boundary is a start boundary.
    pub fn start_boundary_time(&self, cursor: TimeInt) -> TimeInt {
        match *self {
            Self::Absolute(time) => time,
            Self::CursorRelative(time) => cursor + time,
            Self::Infinite => TimeInt::MIN,
        }
    }

    /// Returns the correct time assuming this boundary is an end boundary.
    pub fn end_boundary_time(&self, cursor: TimeInt) -> TimeInt {
        match *self {
            Self::Absolute(time) => time,
            Self::CursorRelative(time) => cursor + time,
            Self::Infinite => TimeInt::MAX,
        }
    }
}
