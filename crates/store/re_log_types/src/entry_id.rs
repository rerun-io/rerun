#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[serde(transparent)]
pub struct EntryId {
    pub id: re_tuid::Tuid,
}

impl Default for EntryId {
    fn default() -> Self {
        Self::new()
    }
}

impl EntryId {
    #[inline]
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
