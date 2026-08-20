/// Reference to any kind of table outside of a recording.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    re_byte_size::SizeBytes,
)]
pub enum TableReference {
    /// A table that exists only locally in the viewer.
    LocalTable(re_log_types::TableId),

    /// The `__entries` table of a remote server.
    // TODO(RR-5454): servers's `__entries` technically has a defined entryid, but we don't know it everywhere and doesn't yet behave consistently when asked for its entry details.
    RedapServerEntries { origin: crate::Origin },

    /// Either a dataset segment table or a dataset entry table on a remote server.
    RedapEntry {
        origin: crate::Origin,
        entry_id: re_log_types::EntryId,
    },
}

impl TableReference {
    pub fn local(table_id: impl Into<re_log_types::TableId>) -> Self {
        Self::LocalTable(table_id.into())
    }

    pub fn url(&self) -> Option<crate::RedapUri> {
        match self {
            Self::LocalTable(_) => None,
            Self::RedapServerEntries { origin } => Some(crate::RedapUri::Catalog(
                crate::CatalogUri::new(origin.clone()),
            )),
            Self::RedapEntry { origin, entry_id } => Some(crate::RedapUri::Entry(
                crate::EntryUri::new(origin.clone(), *entry_id),
            )),
        }
    }
}

impl From<re_log_types::TableId> for TableReference {
    fn from(table_id: re_log_types::TableId) -> Self {
        Self::LocalTable(table_id)
    }
}

impl From<crate::EntryUri> for TableReference {
    fn from(uri: crate::EntryUri) -> Self {
        Self::RedapEntry {
            origin: uri.origin,
            entry_id: uri.entry_id,
        }
    }
}
