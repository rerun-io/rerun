use std::str::FromStr;

/// The id for an entry (i.e. a dataset or a table) in a remote catalog.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EntryId {
    pub id: re_tuid::Tuid,
}

impl EntryId {
    #[inline]
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            id: re_tuid::Tuid::new(),
        }
    }
}

impl std::fmt::Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl From<re_tuid::Tuid> for EntryId {
    fn from(id: re_tuid::Tuid) -> Self {
        Self { id }
    }
}

impl FromStr for EntryId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        re_tuid::Tuid::from_str(s).map(|id| Self { id })
    }
}
