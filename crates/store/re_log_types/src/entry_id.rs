use std::str::FromStr;

/// The id for an entry (i.e. a dataset or a table) in a remote catalog.
///
/// This is the identity of the entry: immutable, randomly generated, and never reused.
/// Store and reference entries by `EntryId`.
///
/// The counterpart is [`EntryName`](crate::EntryName), the human-facing label. A name is
/// unique within a catalog, but it can be changed and then reused by a different entry, so it
/// is only a lookup key, resolved to an `EntryId` at the time of the lookup.
/// [`EntryIdOrName`] exists for APIs that accept either.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    re_byte_size::SizeBytes,
)]
#[serde(transparent)]
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

// ---

/// Either an [`EntryId`] or an [`EntryName`](crate::EntryName) for an entry.
///
/// This helper type should only be used for APIs to offer the convenience to refer to entries by
/// either name or id. For storage/indexing purposes, use [`EntryId`]: a name is mutable and can
/// be reused by another entry after a rename, so it must be resolved to an [`EntryId`] before it
/// is stored anywhere.
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
