use super::{TimeInt, VisibleTimeRangeBoundary, VisibleTimeRangeBoundaryKind};

impl VisibleTimeRangeBoundary {
    /// Put the boundary at the current time cursor.
    pub const AT_CURSOR: Self = Self {
        kind: VisibleTimeRangeBoundaryKind::RelativeToTimeCursor,
        time: TimeInt(0),
    };

    /// The boundary extends to infinity.
    ///
    /// For minimum bounds, this mean the minimum time (-∞),
    /// and for maximum bounds, this means the maximum time (+∞).
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

    /// Creates a new absolute boundary.
    pub fn absolute(time: TimeInt) -> Self {
        Self {
            kind: VisibleTimeRangeBoundaryKind::Absolute,
            time,
        }
    }

    /// Creates a new cursor relative boundary.
    pub fn relative_to_time_cursor(time: TimeInt) -> Self {
        Self {
            kind: VisibleTimeRangeBoundaryKind::RelativeToTimeCursor,
            time,
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
