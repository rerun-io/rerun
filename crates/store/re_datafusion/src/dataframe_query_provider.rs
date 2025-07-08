use arrow::array::{Array, RecordBatch, StringArray, new_null_array};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use async_trait::async_trait;
use datafusion::catalog::Session;
use datafusion::common::{plan_datafusion_err, plan_err};
use datafusion::datasource::TableType;
use datafusion::execution::{RecordBatchStream, TaskContext};
use datafusion::logical_expr::Expr;
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr, PhysicalSortExpr,
};
use datafusion::physical_plan::coalesce_batches::CoalesceBatchesExec;
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use datafusion::{
    catalog::TableProvider, error::DataFusionError, execution::SendableRecordBatchStream,
};
use futures_util::Stream;
use re_dataframe::{Index, QueryEngine, QueryExpression, QueryHandle, StorageEngine};
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Sets the size for output record batches in rows. The last batch will likely be smaller.
/// The default for Data Fusion is 8192, which leads to a 256Kb record batch on average for
/// rows with 32b of data. We are setting this lower as a reasonable first guess to avoid
/// the pitfall of executing a single row at a time, but we will likely want to consider
/// at some point moving to a dynamic sizing.
const DEFAULT_BATCH_SIZE: usize = 2048;

pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_engines: Vec<(String, QueryEngine<StorageEngine>)>,
    query_expression: QueryExpression,
    sort_index: Option<Index>,
}

impl Debug for DataframeQueryTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataframeQueryTableProvider")
            .field("schema", &self.schema)
            .field("sort_index", &self.sort_index)
            .finish()
    }
}

pub struct DataframePartitionStream {
    query_handle: QueryHandle<StorageEngine>,
    partition_id: String,
    projected_schema: SchemaRef,
}

impl DataframeQueryTableProvider {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new(
        query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
        query_expression: &QueryExpression,
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

        let query_engines = query_engines.into_iter().collect();

        Ok(Self {
            schema,
            query_engines,
            query_expression: query_expression.to_owned(),
            sort_index: query_expression.filtered_index,
        })
    }
}

#[async_trait]
impl TableProvider for DataframeQueryTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    #[tracing::instrument(level = "info", skip_all)]
    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        PartitionStreamExec::try_new(
            &self.schema,
            self.sort_index,
            projection,
            self.query_engines.clone(),
            self.query_expression.clone(),
        )
        .map(Arc::new)
        .map(|exec| {
            Arc::new(CoalesceBatchesExec::new(exec, DEFAULT_BATCH_SIZE).with_fetch(limit))
                as Arc<dyn ExecutionPlan>
        })
    }
}

impl Stream for DataframePartitionStream {
    type Item = Result<RecordBatch, DataFusionError>;

    #[tracing::instrument(level = "info", skip_all)]
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let query_handle = &this.query_handle;

        let num_fields = query_handle.schema().fields.len();

        let Some(next_row) = query_handle.next_row() else {
            return Poll::Ready(None);
        };
        if next_row.is_empty() {
            // Should not happen
            return Poll::Ready(None);
        }
        if num_fields != next_row.len() {
            return Poll::Ready(Some(plan_err!(
                "Unexpected number of columns returned from query"
            )));
        }

        let pid_array = Arc::new(StringArray::from(vec![
            this.partition_id.clone();
            next_row[0].len()
        ])) as Arc<dyn Array>;

        let mut arrays = Vec::with_capacity(num_fields + 1);
        arrays.push(pid_array);
        arrays.extend(next_row);

        let batch_schema = Arc::new(prepend_string_column_schema(
            query_handle.schema(),
            DATASET_MANIFEST_ID_FIELD_NAME,
        ));

        let batch = RecordBatch::try_new(batch_schema, arrays)?;

        let output_batch = align_record_batch_to_schema(&batch, &this.projected_schema)?;

        Poll::Ready(Some(Ok(output_batch)))
    }
}

impl RecordBatchStream for DataframePartitionStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.projected_schema)
    }
}

#[tracing::instrument(level = "info", skip_all)]
fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
}

#[tracing::instrument(level = "info", skip_all)]
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

struct PartitionStreamExec {
    props: PlanProperties,
    query_engines: Vec<(String, QueryEngine<StorageEngine>)>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
}

impl Debug for PartitionStreamExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartitionStreamExec")
            .field("props", &self.props)
            .field("projected_schema", &self.projected_schema)
            .finish()
    }
}

impl PartitionStreamExec {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        query_engines: Vec<(String, QueryEngine<StorageEngine>)>,
        query_expression: QueryExpression,
    ) -> datafusion::common::Result<Self> {
        let projected_schema = match projection {
            Some(p) => Arc::new(table_schema.project(p)?),
            None => Arc::clone(table_schema),
        };

        let order_col = sort_index
            .and_then(|index| {
                let index_name = index.as_str();
                projected_schema
                    .fields()
                    .iter()
                    .enumerate()
                    .find(|(_idx, field)| field.name() == index_name)
                    .map(|(index_col, _)| Column::new(index_name, index_col))
            })
            .map(|expr| Arc::new(expr) as Arc<dyn PhysicalExpr>);
        let orderings = match order_col {
            Some(col_expr) => vec![LexOrdering::new(vec![PhysicalSortExpr::new(
                col_expr,
                SortOptions::new(false, true),
            )])],
            None => vec![],
        };

        let eq_properties =
            EquivalenceProperties::new_with_orderings(Arc::clone(&projected_schema), &orderings);

        let partition_in_output_schema = projection.map(|p| p.contains(&0)).unwrap_or(false);

        let output_partitioning = if partition_in_output_schema {
            Partitioning::Hash(
                vec![Arc::new(Column::new(DATASET_MANIFEST_ID_FIELD_NAME, 0))],
                query_engines.len(),
            )
        } else {
            Partitioning::UnknownPartitioning(query_engines.len())
        };

        let props = PlanProperties::new(
            eq_properties,
            output_partitioning,
            EmissionType::Incremental,
            Boundedness::Bounded,
        );

        Ok(Self {
            props,
            query_engines,
            query_expression,
            projected_schema,
        })
    }
}

impl ExecutionPlan for PartitionStreamExec {
    fn name(&self) -> &'static str {
        "PartitionStreamExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &PlanProperties {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        _children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        plan_err!("PartitionStreamExec does not support children")
    }

    #[tracing::instrument(level = "info", skip_all)]
    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        let (partition_id, engine) = self
            .query_engines
            .get(partition)
            .ok_or(plan_datafusion_err!("Invalid partition index"))?;
        let query_handle = engine.query(self.query_expression.clone());

        let stream = DataframePartitionStream {
            query_handle,
            partition_id: partition_id.clone(),
            projected_schema: self.projected_schema.clone(),
        };

        Ok(Box::pin(stream))
    }
}

impl DisplayAs for PartitionStreamExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StreamingTableExec: num_partitions={:?}",
            self.query_engines.len(),
        )
    }
}
