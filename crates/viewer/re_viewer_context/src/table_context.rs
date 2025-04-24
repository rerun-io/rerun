use re_log_types::TableId;

use crate::TableStore;

/// Everything that is required to display a table entry in the UI.
pub struct TableContext<'a> {
    /// The current active table.
    pub table_id: TableId,

    /// The corresponding [`TableStore`]
    pub store: &'a TableStore,
}
