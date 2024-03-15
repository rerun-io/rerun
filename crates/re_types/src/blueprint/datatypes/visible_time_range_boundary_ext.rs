use super::{TimeInt, VisibleTimeRangeBoundary, VisibleTimeRangeBoundaryKind};

impl VisibleTimeRangeBoundary {
    pub const AT_CURSOR: Self = Self {
        kind: VisibleTimeRangeBoundaryKind::RelativeToTimeCursor,
        time: TimeInt(0),
    };

    pub const INFINITE: Self = Self {
        kind: VisibleTimeRangeBoundaryKind::Infinite,
        time: TimeInt(0),
    };

    /// Returns the time assuming this boundary is a start boundary.
    pub fn start_boundary_time(&self, cursor: TimeInt) -> TimeInt {
        match self.kind {
            VisibleTimeRangeBoundaryKind::Absolute => self.time,
            VisibleTimeRangeBoundaryKind::RelativeToTimeCursor => TimeInt(cursor.0 + self.time.0),
            VisibleTimeRangeBoundaryKind::Infinite => TimeInt::MIN,
        }
    }

    /// Returns the correct time assuming this boundary is an end boundary.
    pub fn end_boundary_time(&self, cursor: TimeInt) -> TimeInt {
        match self.kind {
            VisibleTimeRangeBoundaryKind::Absolute => self.time,
            VisibleTimeRangeBoundaryKind::RelativeToTimeCursor => TimeInt(cursor.0 + self.time.0),
            VisibleTimeRangeBoundaryKind::Infinite => TimeInt::MAX,
        }
    }
}

impl PartialEq for VisibleTimeRangeBoundary {
    fn eq(&self, other: &Self) -> bool {
        match self.kind {
            VisibleTimeRangeBoundaryKind::Absolute
            | VisibleTimeRangeBoundaryKind::RelativeToTimeCursor => {
                other.kind == self.kind && other.time == self.time
            }
            // Ignore the time value for infinite boundaries.
            VisibleTimeRangeBoundaryKind::Infinite => other.kind == self.kind,
        }
    }
}

impl Eq for VisibleTimeRangeBoundary {}
