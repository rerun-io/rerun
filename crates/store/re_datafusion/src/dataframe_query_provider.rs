use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow::array::{Array, RecordBatch, StringArray, new_null_array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::{
    catalog::{TableProvider, streaming::StreamingTable},
    error::DataFusionError,
    execution::SendableRecordBatchStream,
    physical_plan::{stream::RecordBatchStreamAdapter, streaming::PartitionStream},
};

use itertools::Itertools;
use re_dataframe::{QueryEngine, QueryExpression, StorageEngine};
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
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

        Ok(Self {
            schema: Arc::new(prepend_string_column_schema(
                &merged,
                DATASET_MANIFEST_ID_FIELD_NAME,
            )),
            query_engines,
            query_expression,
        })
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

        // let partition_id_columns = HashMap::new();

        let target_schema = self.schema.clone();
        let stream = futures_util::stream::iter(engines.into_iter().flat_map(
            move |(partition_id, query_engine)| {
                let inner_schema = target_schema.clone();
                query_engine
                    .query(query_expression.clone())
                    .into_batch_iter()
                    .map(move |batch| {
                        align_record_batch_to_schema(
                            &prepend_string_column(
                                &batch,
                                DATASET_MANIFEST_ID_FIELD_NAME,
                                partition_id.as_str(),
                            )?,
                            &inner_schema,
                        )
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

fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
}

fn prepend_string_column(
    batch: &RecordBatch,
    column_name: &str,
    value: &str,
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let row_count = batch.num_rows();

    let new_array =
        Arc::new(StringArray::from(vec![value.to_owned(); row_count])) as Arc<dyn Array>;

    let fields = std::iter::once(Field::new(column_name, DataType::Utf8, false))
        .chain(batch.schema().fields().iter().map(|f| (**f).clone()))
        .collect_vec();
    let schema = Arc::new(Schema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    let columns = std::iter::once(new_array)
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
