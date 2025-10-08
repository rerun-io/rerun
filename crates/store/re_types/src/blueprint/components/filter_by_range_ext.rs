use super::FilterByRange;
use re_log_types::TimeInt;

impl Default for FilterByRange {
    fn default() -> Self {
        Self::new(TimeInt::MIN, TimeInt::MAX)
    }
}

impl FilterByRange {
    /// Create a new range filter with the provided time boundaries.
    pub fn new(start: TimeInt, end: TimeInt) -> Self {
        FilterByRange(crate::blueprint::datatypes::FilterByRange {
            start: start.into(),
            end: end.into(),
        })
    }
}
