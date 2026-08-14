/// Range & type of chunk store query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryRange {
    /// Use a time range on the currently active timeline.
    ///
    /// This is also known as "visible time range"
    TimeRange(re_sdk_types::encodings::TimeRange),

    /// Use latest-at semantics.
    #[default]
    LatestAt,
}
