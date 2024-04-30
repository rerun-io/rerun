/// Range & type of data store query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QueryRange {
    /// Use a time range on the currently active timeline.
    TimeRange(re_types::blueprint::datatypes::VisibleTimeRange),

    /// Use latest-at semantics.
    #[default]
    LatestAt,
}
