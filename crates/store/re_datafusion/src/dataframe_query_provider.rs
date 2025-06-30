use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, RecordBatch, StringArray, new_null_array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::{
    catalog::{TableProvider, streaming::StreamingTable},
    error::DataFusionError,
    execution::SendableRecordBatchStream,
    physical_plan::{stream::RecordBatchStreamAdapter, streaming::PartitionStream},
};
use itertools::Itertools as _;
use re_dataframe::{QueryEngine, QueryExpression, StorageEngine};
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    // query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
    partition_streams: Vec<Arc<DataframePartitionStream>>,
}

pub struct DataframePartitionStream {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_engine: QueryEngine<StorageEngine>,
    partition_id: String,
}

impl DataframeQueryTableProvider {
    pub fn new(
        query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
        query_expression: QueryExpression,
    ) -> Result<Self, DataFusionError> {
        let all_schemas = query_engines
            .values()
            .map(|engine| (**engine.query(query_expression.clone()).schema()).clone())
            .collect::<Vec<_>>();

        let merged = Schema::try_merge(all_schemas)?;
        let schema = Arc::new(prepend_string_column_schema(
            &merged,
            DATASET_MANIFEST_ID_FIELD_NAME,
        ));

        let partition_streams = query_engines
            .into_iter()
            .map(|(partition_id, query_engine)| DataframePartitionStream {
                schema: Arc::clone(&schema),
                query_expression: query_expression.clone(),
                query_engine,
                partition_id,
            })
            .map(Arc::new)
            .collect();

        Ok(Self {
            schema,
            query_expression,
            partition_streams,
        })
    }
}

impl TryFrom<DataframeQueryTableProvider> for Arc<dyn TableProvider> {
    type Error = DataFusionError;

    fn try_from(value: DataframeQueryTableProvider) -> Result<Self, Self::Error> {
        let schema = Arc::clone(&value.schema);

        let partition_streams = value
            .partition_streams
            .into_iter()
            .map(|p| p as Arc<dyn PartitionStream>)
            .collect();
        let table = StreamingTable::try_new(schema, partition_streams)?;

        Ok(Arc::new(table))
    }
}

impl PartitionStream for DataframePartitionStream {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<datafusion::execution::TaskContext>) -> SendableRecordBatchStream {
        let partition_id = self.partition_id.clone();
        let query_engine = self.query_engine.clone();
        let query_expression = self.query_expression.clone();

        let mut partition_id_columns: HashMap<String, (Field, ArrayRef)> = HashMap::new();

        let target_schema = self.schema.clone();

        let stream = futures_util::stream::iter({
            let (pid_field, pid_array) = partition_id_columns
                .entry(partition_id.clone())
                .or_insert_with(|| {
                    // The batches returned by re_dataframe are guaranteed to always have a
                    // single row in them. This will change some day, hopefully, but it's been
                    // true for years so we should at least leverage it for now.
                    (
                        Field::new(DATASET_MANIFEST_ID_FIELD_NAME, DataType::Utf8, false),
                        Arc::new(StringArray::from(vec![partition_id.clone()])) as Arc<dyn Array>,
                    )
                });
            let (pid_field, pid_array) = (pid_field.clone(), pid_array.clone());

            let inner_schema = target_schema.clone();
            query_engine
                .query(query_expression.clone())
                .into_batch_iter()
                .map(move |batch| {
                    align_record_batch_to_schema(
                        &prepend_partition_id_column(&batch, pid_field.clone(), pid_array.clone())?,
                        &inner_schema,
                    )
                })
        });

        let adapter = RecordBatchStreamAdapter::new(Arc::clone(&self.schema), stream);

        Box::pin(adapter)
    }
}

impl std::fmt::Debug for DataframePartitionStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataframePartitionStream")
            .field("schema", &self.schema)
            .field("query_expression", &self.query_expression)
            .finish()
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

fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
}

fn prepend_partition_id_column(
    batch: &RecordBatch,
    partition_id_field: Field,
    partition_id_column: ArrayRef,
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let fields = std::iter::once(partition_id_field)
        .chain(batch.schema().fields().iter().map(|f| (**f).clone()))
        .collect_vec();
    let schema = Arc::new(Schema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    let columns = std::iter::once(partition_id_column)
        .chain(batch.columns().iter().cloned())
        .collect_vec();

    RecordBatch::try_new(schema, columns)
}

pub fn align_record_batch_to_schema(
    batch: &RecordBatch,
    target_schema: &Arc<Schema>,
) -> Result<RecordBatch, DataFusionError> {
    let num_rows = batch.num_rows();

    let mut aligned_columns = Vec::with_capacity(target_schema.fields().len());

    for field in target_schema.fields() {
        if let Some((idx, _)) = batch.schema().column_with_name(field.name()) {
            aligned_columns.push(batch.column(idx).clone());
        } else {
            // Fill with nulls of the right data type
            let array = new_null_array(field.data_type(), num_rows);
            aligned_columns.push(array);
        }
    }

    Ok(RecordBatch::try_new(
        target_schema.clone(),
        aligned_columns,
    )?)
}
