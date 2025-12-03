use std::sync::Arc;

use arrow::array::RecordBatch;
use datafusion::common::DataFusionError;
use datafusion::datasource::MemTable;
use datafusion::prelude::SessionContext;
use re_chunk::external::re_byte_size::SizeBytes as _;
use re_log_types::TableId;

#[derive(Default)]
pub struct TableStore {
    record_batches: parking_lot::RwLock<Vec<RecordBatch>>,
    session_ctx: Arc<SessionContext>,
}

impl TableStore {
    pub const TABLE_NAME: &'static str = "__table__";

    pub fn session_context(&self) -> Arc<SessionContext> {
        self.session_ctx.clone()
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.record_batches
            .read()
            .iter()
            .map(|record_batch| record_batch.total_size_bytes())
            .sum()
    }

    pub fn add_record_batch(&self, record_batch: RecordBatch) -> Result<(), DataFusionError> {
        let schema = record_batch.schema();
        self.session_ctx.deregister_table(Self::TABLE_NAME).ok();

        let mut record_batches = self.record_batches.write();
        record_batches.push(record_batch);

        let table = MemTable::try_new(schema, vec![record_batches.clone()])?;

        self.session_ctx
            .register_table(Self::TABLE_NAME, Arc::new(table))?;

        Ok(())
    }
}

pub type TableStores = ahash::HashMap<TableId, TableStore>;
