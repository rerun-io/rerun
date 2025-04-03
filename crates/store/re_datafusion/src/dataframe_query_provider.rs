use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use datafusion::{
    catalog::{streaming::StreamingTable, TableProvider},
    error::DataFusionError,
    execution::SendableRecordBatchStream,
    physical_plan::{stream::RecordBatchStreamAdapter, streaming::PartitionStream},
};
use futures_util::StreamExt as _;

use re_dataframe::{QueryEngine, QueryExpression, StorageEngine};

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_engine: QueryEngine<StorageEngine>,
}

impl DataframeQueryTableProvider {
    pub fn new(
        query_engine: QueryEngine<StorageEngine>,
        query_expression: QueryExpression,
    ) -> Self {
        let schema = query_engine
            .query(query_expression.clone())
            .schema()
            .clone();

        Self {
            schema,
            query_engine,
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
        let stream = futures_util::stream::iter(
            self.query_engine
                .query(self.query_expression.clone())
                .into_batch_iter(),
        )
        .map(Ok);
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
