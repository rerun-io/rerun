use super::VisibleTimeRange;

impl VisibleTimeRange {
    pub const EMPTY: Self = Self(crate::blueprint::datatypes::VisibleTimeRange::EMPTY);
    pub const EVERYTHING: Self = Self(crate::blueprint::datatypes::VisibleTimeRange::EVERYTHING);
}
