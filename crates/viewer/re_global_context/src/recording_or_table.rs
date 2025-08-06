use re_log_types::{StoreId, TableId};

#[derive(Clone, Debug)]
pub enum RecordingOrTable {
    Recording {
        store_id: StoreId,
        // TODO(grtlr): Add `applicationId` here.
    },
    Table {
        table_id: TableId,
    },
}

impl From<StoreId> for RecordingOrTable {
    fn from(store_id: StoreId) -> Self {
        Self::Recording { store_id }
    }
}

impl From<TableId> for RecordingOrTable {
    fn from(table_id: TableId) -> Self {
        Self::Table { table_id }
    }
}

impl RecordingOrTable {
    pub fn recording_ref(&self) -> Option<&StoreId> {
        match self {
            Self::Recording { store_id } => Some(store_id),
            Self::Table { .. } => None,
        }
    }

    pub fn table_ref(&self) -> Option<&TableId> {
        match self {
            Self::Table { table_id } => Some(table_id),
            Self::Recording { .. } => None,
        }
    }
}
