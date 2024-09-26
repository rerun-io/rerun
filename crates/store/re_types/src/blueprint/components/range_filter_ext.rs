use super::RangeFilter;
use re_log_types::TimeInt;

impl Default for RangeFilter {
    fn default() -> Self {
        Self::new(TimeInt::MIN, TimeInt::MAX)
    }
}

impl RangeFilter {
    /// Create a new range filter with the provided time boundaries.
    pub fn new(start: TimeInt, end: TimeInt) -> Self {
        Self(crate::blueprint::datatypes::RangeFilter {
            start: start.into(),
            end: end.into(),
        })
    }
}
