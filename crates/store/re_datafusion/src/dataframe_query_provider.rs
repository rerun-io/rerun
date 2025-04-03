use std::sync::Arc;

use datafusion::{catalog::TableProvider, datasource::MemTable, error::Result as DataFusionResult};
use re_chunk_store::ChunkStore;
use re_dataframe::{ChunkStoreHandle, QueryEngine, QueryExpression};

pub struct DataframeQueryTableProvider {
    query_expression: QueryExpression,
    chunk_store: ChunkStoreHandle,
}

impl DataframeQueryTableProvider {
    pub fn new(chunk_store: ChunkStore, query_expression: QueryExpression) -> Self {
        Self {
            chunk_store: ChunkStoreHandle::new(chunk_store),
            query_expression,
        }
    }

    pub fn create_table(&self) -> DataFusionResult<Arc<dyn TableProvider>> {
        let cache = re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(
            self.chunk_store.clone(),
        ));

        let engine = QueryEngine::new(self.chunk_store.clone(), cache);

        let query_handle = engine.query(self.query_expression.clone());

        let schema = Arc::clone(query_handle.schema());

        let batches: Vec<_> = query_handle.into_batch_iter().collect();

        let table = MemTable::try_new(schema, vec![batches])?;

        Ok(Arc::new(table))
    }
}
