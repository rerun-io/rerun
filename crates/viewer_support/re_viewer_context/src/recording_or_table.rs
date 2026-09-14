use re_log_types::{StoreId, TableId};

use crate::Route;

#[derive(Clone, Debug)]
pub enum RecordingOrLocalTable {
    Recording {
        store_id: StoreId,
        // TODO(grtlr): Add `applicationId` here.
    },
    LocalTable {
        table_id: TableId,
    },
}

impl From<StoreId> for RecordingOrLocalTable {
    fn from(store_id: StoreId) -> Self {
        Self::Recording { store_id }
    }
}

impl From<TableId> for RecordingOrLocalTable {
    fn from(table_id: TableId) -> Self {
        Self::LocalTable { table_id }
    }
}

impl RecordingOrLocalTable {
    pub fn recording_ref(&self) -> Option<&StoreId> {
        match self {
            Self::Recording { store_id } => Some(store_id),
            Self::LocalTable { .. } => None,
        }
    }

    pub fn table_ref(&self) -> Option<&TableId> {
        match self {
            Self::LocalTable { table_id } => Some(table_id),
            Self::Recording { .. } => None,
        }
    }

    /// The route this would equate to.
    pub fn route(&self) -> Route {
        match self {
            Self::Recording { store_id } => Route::LocalRecording {
                recording_id: store_id.clone(),
            },
            Self::LocalTable { table_id } => Route::LocalTable(table_id.clone()),
        }
    }
}
