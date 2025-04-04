use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::{
    catalog::{streaming::StreamingTable, TableProvider},
    error::DataFusionError,
    execution::SendableRecordBatchStream,
    physical_plan::{stream::RecordBatchStreamAdapter, streaming::PartitionStream},
};

use re_dataframe::{QueryEngine, QueryExpression, StorageEngine};

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
}

impl DataframeQueryTableProvider {
    pub fn new(
        query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
        query_expression: QueryExpression,
    ) -> Self {
        let schema = query_engines
            .first_key_value()
            .map(|(_, query_engine)| {
                query_engine
                    .query(query_expression.clone())
                    .schema()
                    .clone()
            })
            .unwrap_or_else(|| Arc::new(arrow::datatypes::Schema::empty()));

        Self {
            schema,
            query_engines,
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
        let engines = self.query_engines.clone();
        let query_expression = self.query_expression.clone();

        let stream = futures_util::stream::iter(engines.into_iter().flat_map(
            move |(partition_id, query_engine)| {
                query_engine
                    .query(query_expression.clone())
                    .into_batch_iter()
                    .map(move |batch| {
                        prepend_string_column(&batch, "__partition_id", partition_id.as_str())
                            .map_err(Into::into)
                    })
            },
        ));

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

fn prepend_string_column(
    batch: &RecordBatch,
    column_name: &str,
    value: &str,
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let row_count = batch.num_rows();

    let new_array =
        Arc::new(StringArray::from(vec![value.to_owned(); row_count])) as Arc<dyn Array>;

    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(batch.schema().fields().iter().map(|f| (**f).clone()));
    let schema = Arc::new(Schema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    let mut columns = vec![new_array];
    columns.extend(batch.columns().iter().cloned());

    RecordBatch::try_new(schema, columns)
}
