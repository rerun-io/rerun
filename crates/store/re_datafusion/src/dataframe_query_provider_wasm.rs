use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::DataframeClientAPI;
use crate::dataframe_query_common::group_chunk_infos_by_segment_id;
use arrow::array::{Array, RecordBatch, RecordBatchOptions, StringArray};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::hash_utils::HashValue as _;
use datafusion::common::{exec_datafusion_err, exec_err, plan_err};
use datafusion::config::ConfigOptions;
use datafusion::error::DataFusionError;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr, PhysicalSortExpr,
};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use futures_util::{Stream, StreamExt as _};
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{
    ChunkStoreHandle, Index, QueryCache, QueryEngine, QueryExpression, QueryHandle, StorageEngine,
};
use re_log_types::{StoreId, StoreKind};
use re_protos::cloud::v1alpha1::{FetchChunksRequest, ScanSegmentTableResponse};
use tokio::runtime::Handle;
use tonic::IntoRequest as _;

#[derive(Debug)]
pub(crate) struct SegmentStreamExec<T: DataframeClientAPI> {
    props: PlanProperties,
    chunk_info_batches: Arc<Vec<RecordBatch>>,

    /// Describes the chunks per segment, derived from `chunk_info_batches`.
    /// We keep both around so that we only have to process once, but we may
    /// reuse multiple times in theory. We may also need to recompute if the
    /// user asks for a different target partition. These are generally not
    /// too large.
    chunk_info: Arc<BTreeMap<String, Vec<RecordBatch>>>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
    target_partitions: usize,
    client: T,
}

pub struct DataframeSegmentStream<T: DataframeClientAPI> {
    projected_schema: SchemaRef,
    client: T,
    chunk_infos: Vec<RecordBatch>,
    current_query: Option<(String, QueryHandle<StorageEngine>)>,
    query_expression: QueryExpression,
    remaining_segment_ids: Vec<String>,
}

impl<T: DataframeClientAPI> DataframeSegmentStream<T> {
    async fn get_chunk_store_for_single_rerun_segment(
        &mut self,
        segment_id: &str,
    ) -> Result<ChunkStoreHandle, DataFusionError> {
        let chunk_infos = self.chunk_infos.iter().map(Into::into).collect::<Vec<_>>();
        let fetch_chunks_request = FetchChunksRequest { chunk_infos };

        let fetch_chunks_response_stream = self
            .client
            .fetch_chunks(fetch_chunks_request.into_request())
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner();

        // Then we need to fully decode these chunks, i.e. both the transport layer (Protobuf)
        // and the app layer (Arrow).
        let mut chunk_stream = re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(
            fetch_chunks_response_stream,
        );

        // Note: using segment id as the store id, shouldn't really
        // matter since this is just a temporary store.
        let store_id = StoreId::random(StoreKind::Recording, segment_id);
        let store = ChunkStore::new_handle(store_id, Default::default());

        while let Some(chunks_and_segment_ids) = chunk_stream.next().await {
            let chunks_and_segment_ids =
                chunks_and_segment_ids.map_err(|err| exec_datafusion_err!("{err}"))?;

            let _span = tracing::trace_span!(
                "fetch_chunks::batch_insert",
                num_chunks = chunks_and_segment_ids.len()
            )
            .entered();

            for chunk_and_segment_id in chunks_and_segment_ids {
                let (chunk, received_segment_id) = chunk_and_segment_id;

                let received_segment_id = received_segment_id
                    .ok_or_else(|| exec_datafusion_err!("Received chunk without a segment id"))?;
                if received_segment_id != segment_id {
                    return exec_err!("Unexpected segment id: {received_segment_id}");
                }

                store
                    .write()
                    .insert_chunk(&Arc::new(chunk))
                    .map_err(|err| exec_datafusion_err!("{err}"))?;
            }
        }

        Ok(store)
    }
}

impl<T: DataframeClientAPI> Stream for DataframeSegmentStream<T> {
    type Item = Result<RecordBatch, DataFusionError>;

    #[tracing::instrument(level = "info", skip_all)]
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if this.remaining_segment_ids.is_empty() && this.current_query.is_none() {
                return Poll::Ready(None);
            }

            while this.current_query.is_none() {
                let Some(segment_id) = this.remaining_segment_ids.pop() else {
                    return Poll::Ready(None);
                };

                let runtime = Handle::current();
                let store = runtime
                    .block_on(this.get_chunk_store_for_single_rerun_segment(segment_id.as_str()))?;

                let query_engine = QueryEngine::new(store.clone(), QueryCache::new_handle(store));

                let query = query_engine.query(this.query_expression.clone());

                if query.num_rows() > 0 {
                    this.current_query = Some((segment_id, query));
                }
            }

            let (segment_id, query) = this
                .current_query
                .as_mut()
                .expect("current_query should be Some");

            // If the following returns none, we have exhausted that rerun segment id
            match create_next_row(query, segment_id, &this.projected_schema)? {
                Some(rb) => return Poll::Ready(Some(Ok(rb))),
                None => this.current_query = None,
            }
        }
    }
}

impl<T: DataframeClientAPI> RecordBatchStream for DataframeSegmentStream<T> {
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

impl<T: DataframeClientAPI> SegmentStreamExec<T> {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        num_partitions: usize,
        chunk_info_batches: Arc<Vec<RecordBatch>>,
        query_expression: QueryExpression,
        client: T,
    ) -> datafusion::common::Result<Self> {
        let projected_schema = match projection {
            Some(p) => Arc::new(table_schema.project(p)?),
            None => Arc::clone(table_schema),
        };

        let partition_col = Arc::new(Column::new(ScanSegmentTableResponse::FIELD_SEGMENT_ID, 0))
            as Arc<dyn PhysicalExpr>;
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

        let mut physical_ordering = vec![PhysicalSortExpr::new(
            partition_col,
            SortOptions::new(false, true),
        )];
        if let Some(col_expr) = order_col {
            physical_ordering.push(PhysicalSortExpr::new(
                col_expr,
                SortOptions::new(false, true),
            ));
        }

        let orderings = vec![
            LexOrdering::new(physical_ordering)
                .expect("LexOrdering should return Some when non-empty vec is passed"),
        ];

        let eq_properties =
            EquivalenceProperties::new_with_orderings(Arc::clone(&projected_schema), orderings);

        let partition_in_output_schema = projection.map(|p| p.contains(&0)).unwrap_or(false);

        let output_partitioning = if partition_in_output_schema {
            Partitioning::Hash(
                vec![Arc::new(Column::new(
                    ScanSegmentTableResponse::FIELD_SEGMENT_ID,
                    0,
                ))],
                num_partitions,
            )
        } else {
            Partitioning::UnknownPartitioning(num_partitions)
        };

        let props = PlanProperties::new(
            eq_properties,
            output_partitioning,
            EmissionType::Incremental,
            Boundedness::Bounded,
        );

        let chunk_info = group_chunk_infos_by_segment_id(&chunk_info_batches)?;

        Ok(Self {
            props,
            chunk_info_batches,
            chunk_info,
            query_expression,
            projected_schema,
            target_partitions: num_partitions,
            client,
        })
    }
}

#[tracing::instrument(level = "trace", skip_all)]
fn create_next_row(
    query_handle: &QueryHandle<StorageEngine>,
    segment_id: &str,
    target_schema: &Arc<Schema>,
) -> Result<Option<RecordBatch>, DataFusionError> {
    let query_schema = Arc::clone(query_handle.schema());
    let num_fields = query_schema.fields.len();

    let Some(next_row) = query_handle.next_row() else {
        return Ok(None);
    };

    if next_row.is_empty() {
        // Should not happen
        return Ok(None);
    }
    if num_fields != next_row.len() {
        return plan_err!("Unexpected number of columns returned from query");
    }

    let num_rows = next_row[0].len();
    let sid_array =
        Arc::new(StringArray::from(vec![segment_id.to_owned(); num_rows])) as Arc<dyn Array>;

    let mut arrays = Vec::with_capacity(num_fields + 1);
    arrays.push(sid_array);
    arrays.extend(next_row);

    let batch_schema = Arc::new(prepend_string_column_schema(
        &query_schema,
        ScanSegmentTableResponse::FIELD_SEGMENT_ID,
    ));

    let batch = RecordBatch::try_new_with_options(
        batch_schema,
        arrays,
        &RecordBatchOptions::default().with_row_count(Some(num_rows)),
    )?;

    let output_batch = align_record_batch_to_schema(&batch, target_schema)?;

    Ok(Some(output_batch))
}

impl<T: DataframeClientAPI> ExecutionPlan for SegmentStreamExec<T> {
    fn name(&self) -> &'static str {
        "SegmentStreamExec"
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
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        if children.is_empty() {
            Ok(self)
        } else {
            plan_err!("SegmentStreamExec does not support children")
        }
    }

    fn repartitioned(
        &self,
        target_partitions: usize,
        _config: &ConfigOptions,
    ) -> datafusion::common::Result<Option<Arc<dyn ExecutionPlan>>> {
        if target_partitions == self.target_partitions {
            return Ok(None);
        }

        let mut plan = Self {
            props: self.props.clone(),
            chunk_info_batches: self.chunk_info_batches.clone(),
            chunk_info: self.chunk_info.clone(),
            query_expression: self.query_expression.clone(),
            projected_schema: self.projected_schema.clone(),
            target_partitions,
            client: self.client.clone(),
        };

        plan.props.partitioning = match plan.props.partitioning {
            Partitioning::RoundRobinBatch(_) => Partitioning::RoundRobinBatch(target_partitions),
            Partitioning::UnknownPartitioning(_) => {
                Partitioning::UnknownPartitioning(target_partitions)
            }
            Partitioning::Hash(expr, _) => Partitioning::Hash(expr, target_partitions),
        };

        Ok(Some(Arc::new(plan) as Arc<dyn ExecutionPlan>))
    }

    #[tracing::instrument(level = "info", skip_all)]
    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let mut remaining_segment_ids = self
            .chunk_info
            .keys()
            .filter(|segment_id| {
                let hash_value = segment_id.hash_one(&random_state) as usize;
                hash_value % self.target_partitions == partition
            })
            .cloned()
            .collect::<Vec<_>>();
        remaining_segment_ids.sort();
        remaining_segment_ids.reverse();

        let client = self.client.clone();

        let chunk_infos: Vec<RecordBatch> = remaining_segment_ids
            .iter()
            .filter_map(|sid| self.chunk_info.get(sid))
            .flatten()
            .cloned()
            .collect();

        let query_expression = self.query_expression.clone();

        let stream = DataframeSegmentStream {
            projected_schema: self.projected_schema.clone(),
            client,
            chunk_infos,
            remaining_segment_ids,
            current_query: None,
            query_expression,
        };

        Ok(Box::pin(stream))
    }
}

impl<T: DataframeClientAPI> DisplayAs for SegmentStreamExec<T> {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SegmentStreamExec: num_partitions={:?}",
            self.target_partitions,
        )
    }
}
