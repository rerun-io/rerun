/// Range & type of chunk store query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryRange {
    /// Use a time range on the currently active timeline.
    TimeRange(re_sdk_types::datatypes::TimeRange),

    /// Use latest-at semantics.
    #[default]
    LatestAt,
}

impl QueryRange {
    #[inline]
    pub fn is_latest_at(&self) -> bool {
        matches!(self, Self::LatestAt)
    }

    #[inline]
    pub fn is_time_range(&self) -> bool {
        matches!(self, Self::TimeRange(_))
    }
}
