use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use datafusion::{
    catalog::{streaming::StreamingTable, TableProvider},
    error::DataFusionError,
    execution::SendableRecordBatchStream,
    physical_plan::{stream::RecordBatchStreamAdapter, streaming::PartitionStream},
};
use futures_util::StreamExt;
use re_chunk_store::ChunkStore;
use re_dataframe::{ChunkStoreHandle, QueryEngine, QueryExpression, QueryHandle, StorageEngine};

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    chunk_store: ChunkStoreHandle,
}

fn create_query_handle(
    query_expression: &QueryExpression,
    chunk_store: &ChunkStoreHandle,
) -> QueryHandle<StorageEngine> {
    let cache =
        re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(chunk_store.clone()));

    let engine = QueryEngine::new(chunk_store.clone(), cache);
    engine.query(query_expression.clone())
}

impl DataframeQueryTableProvider {
    pub fn new(chunk_store: ChunkStore, query_expression: QueryExpression) -> Self {
        let chunk_store = ChunkStoreHandle::new(chunk_store);
        let schema = create_query_handle(&query_expression, &chunk_store)
            .schema()
            .to_owned();
        Self {
            schema,
            chunk_store,
            query_expression,
        }
    }
}

impl TryFrom<DataframeQueryTableProvider> for Arc<dyn TableProvider> {
    type Error = DataFusionError;

    fn try_from(value: DataframeQueryTableProvider) -> Result<Self, Self::Error> {
        let schema = Arc::clone(&value.schema);
        let partition_stream = Arc::new(value);
        let table = StreamingTable::try_new(schema, vec![partition_stream])?;

        Ok(Arc::new(table))
    }
}

impl PartitionStream for DataframeQueryTableProvider {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<datafusion::execution::TaskContext>) -> SendableRecordBatchStream {
        let query_handle = create_query_handle(&self.query_expression, &self.chunk_store);

        let stream = futures_util::stream::iter(query_handle.into_batch_iter()).map(|v| (Ok(v)));
        let adapter = RecordBatchStreamAdapter::new(Arc::clone(&self.schema), stream);
        Box::pin(adapter)
    }
}

impl std::fmt::Debug for DataframeQueryTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataframeQueryTableProvider")
            .field("schema", &self.schema)
            .field("query_expression", &self.query_expression)
            .finish()
    }
}
