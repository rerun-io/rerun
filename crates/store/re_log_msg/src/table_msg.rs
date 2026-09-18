use arrow::array::RecordBatch as ArrowRecordBatch;
use re_log_types::TableId;

/// A table, encoded as a single Arrow record batch.
///
/// Tables have a [`TableId`], but don't belong to an application and therefore don't have a [`re_log_types::ApplicationId`].
/// For now, the table is always sent as a whole, i.e. tables can't be streamed.
///
/// It's important to note that tables are not sent via the smart channel of [`crate::LogMsg`], but use a separate `crossbeam`
/// channel. The reasoning behind this is that tables are fundamentally different from recordings. For example,
/// we don't want to store tables in `.rrd` files, as there are much better formats out there.
#[must_use]
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct TableMsg {
    /// The id of the table.
    pub id: TableId,

    /// The table stored as an [`ArrowRecordBatch`].
    pub data: ArrowRecordBatch,
}

impl TableMsg {
    /// Records the current time as the moment this table passed `location`.
    pub fn track_latency(&mut self, location: re_sorbet::TimestampLocation) {
        if let Some(key) = location.metadata_key() {
            self.data.schema_metadata_mut().insert(
                key.to_owned(),
                re_sorbet::timestamp_metadata::now_timestamp(),
            );
        }
    }
}
