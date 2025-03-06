use crate::{NonMinI64, TimeType};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
/// An typed cell of an index, e.g. a point in time on some unknown timeline.
pub struct IndexCell {
    pub typ: TimeType,
    pub value: NonMinI64,
}
