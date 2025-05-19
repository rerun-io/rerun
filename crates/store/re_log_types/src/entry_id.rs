use std::str::FromStr;

/// The id for an entry (i.e. a dataset or a table) in a remote catalog.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct  EntryId {
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

// ---

/// Either an id or a name for an entry.
///
/// This helper type should only be used for APIs to offer the convenience to refer to entries by
/// either name or id. For storage/indexing purposes, use [`EntryId`].
#[derive(Debug, Clone)]
pub enum EntryIdOrName {
    Id(EntryId),
    Name(String),
}

impl From<EntryId> for EntryIdOrName {
    fn from(id: EntryId) -> Self {
        Self::Id(id)
    }
}

impl From<&str> for EntryIdOrName {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for EntryIdOrName {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}
