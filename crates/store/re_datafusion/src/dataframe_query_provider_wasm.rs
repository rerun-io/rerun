use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, atomic::AtomicBool};
use std::task::{Context, Poll};

use crate::DataframeClientAPI;
use crate::dataframe_query_common::{IndexValuesMap, PlanSummary, group_chunk_infos_by_segment_id};
use arrow::array::{Array, RecordBatch, RecordBatchOptions, StringArray};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::hash_utils::HashValue as _;
use datafusion::common::plan_err;
use datafusion::config::ConfigOptions;
use datafusion::error::DataFusionError;
use re_redap_client::{ApiError, ApiResult};

use crate::IntoDfError as _;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr, PhysicalSortExpr,
};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::metrics::MetricsSet;

use crate::analytics::build_metrics_set_for_explain;
use crate::metrics_capture::QueryMetrics;
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
    props: Arc<PlanProperties>,

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

    /// Pending query analytics — always present; the OTLP send on drop is
    /// gated internally by whether the per-process telemetry stack is active.
    pending_analytics: crate::PendingQueryAnalytics,

    /// Per-query counters + embedded plan-time `QueryInfo`. The wasm path
    /// doesn't run a per-partition IO loop with `TaskFetchStats`, so the
    /// fetch counters stay at zero; the embedded `query_info` is what feeds
    /// the snapshot path and `EXPLAIN ANALYZE`.
    metrics: Arc<QueryMetrics>,

    /// Plan-time summary used by `DisplayAs::Verbose`.
    plan_summary: PlanSummary,

    /// Subscribers captured at plan-construction time (from `query_metrics()`).
    captured_collectors: Vec<crate::MetricsCollector>,

    /// Latched true the first time a snapshot is sent.
    snapshot_sent: Arc<AtomicBool>,
}

pub struct DataframeSegmentStream<T: DataframeClientAPI> {
    projected_schema: SchemaRef,
    client: T,
    chunk_infos: Vec<RecordBatch>,
    current_query: Option<(String, QueryHandle<StorageEngine>)>,
    query_expression: QueryExpression,
    remaining_segment_ids: Vec<String>,

    /// Pending query analytics — kept alive so the event fires on drop.
    pending_analytics: crate::PendingQueryAnalytics,

    /// Subscribers captured by the parent plan.
    captured_collectors: Vec<crate::MetricsCollector>,

    /// Shared latch — see `SegmentStreamExec::snapshot_sent`.
    snapshot_sent: Arc<AtomicBool>,

    /// Shared metrics handle used by the snapshot path.
    metrics: Arc<QueryMetrics>,
}

impl<T: DataframeClientAPI> DataframeSegmentStream<T> {
    async fn get_chunk_store_for_single_rerun_segment(
        &mut self,
        segment_id: &str,
    ) -> ApiResult<ChunkStoreHandle> {
        let chunk_infos = self.chunk_infos.iter().map(Into::into).collect::<Vec<_>>();
        let fetch_chunks_request = FetchChunksRequest { chunk_infos };

        let mut req = fetch_chunks_request.into_request();
        req.set_timeout(re_redap_client::FETCH_CHUNKS_DEADLINE);
        let response = self
            .client
            .fetch_chunks(req)
            .await
            .map_err(|err| ApiError::tonic(err, "fetch_chunks"))?;

        let response_stream =
            re_redap_client::ApiResponseStream::from_tonic_response(response, "/FetchChunks");

        // Then we need to fully decode these chunks, i.e. both the transport layer (Protobuf)
        // and the app layer (Arrow).
        let mut chunk_stream =
            re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(response_stream);

        // Note: using segment id as the store id, shouldn't really
        // matter since this is just a temporary store.
        let store_id = StoreId::random(StoreKind::Recording, segment_id);
        let store = ChunkStore::new_handle(store_id, Default::default());

        while let Some(chunks_and_segment_ids) = chunk_stream.next().await {
            let chunks_and_segment_ids = chunks_and_segment_ids?;

            let _span = tracing::trace_span!(
                "fetch_chunks::batch_insert",
                num_chunks = chunks_and_segment_ids.len()
            )
            .entered();

            for chunk_and_segment_id in chunks_and_segment_ids {
                let (chunk, received_segment_id) = chunk_and_segment_id;

                let received_segment_id = received_segment_id.ok_or_else(|| {
                    ApiError::deserialization(
                        None,
                        "server returned chunk without a segment id in fetch_chunks response",
                    )
                })?;
                if received_segment_id.as_ref() != segment_id {
                    return Err(ApiError::deserialization(
                        None,
                        format!(
                            "server returned chunk for unexpected segment id `{received_segment_id}` \
                             while fetching chunks for `{segment_id}`"
                        ),
                    ));
                }

                store
                    .write()
                    .insert_chunk(&Arc::new(chunk))
                    .map_err(|err| {
                        ApiError::internal_with_source(
                            None,
                            err,
                            "inserting chunk into in-memory store",
                        )
                    })?;
            }
        }

        Ok(store)
    }
}

impl<T: DataframeClientAPI> DataframeSegmentStream<T> {
    /// Emit a `QuerySnapshot` to `query_metrics()` subscribers if this is the
    /// first time we've reached end-of-stream for this plan. Idempotent.
    fn maybe_emit_snapshot(&self) {
        if self.captured_collectors.is_empty() {
            return;
        }
        if self
            .snapshot_sent
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        let snapshot = crate::metrics_capture::build_query_snapshot(
            &self.metrics,
            self.pending_analytics.total_duration(),
            self.pending_analytics.time_to_first_chunk(),
            self.pending_analytics.error_kind(),
            self.pending_analytics.direct_terminal_reason(),
        );
        crate::metrics_capture::push_snapshot(&self.captured_collectors, &snapshot);
    }
}

impl<T: DataframeClientAPI> Drop for DataframeSegmentStream<T> {
    fn drop(&mut self) {
        // Cover the cancelled-mid-flight case — `poll_next` may have returned
        // `None` first, in which case the CAS no-ops.
        self.maybe_emit_snapshot();
    }
}

impl<T: DataframeClientAPI> Stream for DataframeSegmentStream<T> {
    type Item = Result<RecordBatch, DataFusionError>;

    #[tracing::instrument(level = "info", skip_all)]
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if this.remaining_segment_ids.is_empty() && this.current_query.is_none() {
                this.maybe_emit_snapshot();
                return Poll::Ready(None);
            }

            while this.current_query.is_none() {
                let Some(segment_id) = this.remaining_segment_ids.pop() else {
                    this.maybe_emit_snapshot();
                    return Poll::Ready(None);
                };

                let runtime = Handle::current();
                let store = runtime
                    .block_on(this.get_chunk_store_for_single_rerun_segment(segment_id.as_str()))
                    .map_err(|err| err.into_df_error())?;

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
            match create_next_row(query, segment_id, &this.projected_schema)
                .map_err(|err| err.into_df_error())?
            {
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
        chunk_info_batches: Option<RecordBatch>,
        query_expression: QueryExpression,
        _index_values: IndexValuesMap,
        client: T,
        _limit: Option<usize>,
        pending_analytics: crate::PendingQueryAnalytics,
        metrics: Arc<QueryMetrics>,
        captured_collectors: Vec<crate::MetricsCollector>,
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
        )
        .into();

        let chunk_info = group_chunk_infos_by_segment_id(chunk_info_batches.as_slice())?;
        drop(chunk_info_batches);

        let plan_summary = PlanSummary::from_query_info(&metrics.query_info);

        let snapshot_sent = Arc::new(AtomicBool::new(false));

        Ok(Self {
            props,
            chunk_info,
            query_expression,
            projected_schema,
            target_partitions: num_partitions,
            client,
            pending_analytics,
            metrics,
            plan_summary,
            captured_collectors,
            snapshot_sent,
        })
    }
}

#[tracing::instrument(level = "trace", skip_all)]
fn create_next_row(
    query_handle: &mut QueryHandle<StorageEngine>,
    segment_id: &str,
    target_schema: &Arc<Schema>,
) -> ApiResult<Option<RecordBatch>> {
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
        return Err(ApiError::internal(
            "Unexpected number of columns returned from query",
        ));
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
    )
    .map_err(|err| {
        ApiError::deserialization_with_source(
            None,
            err,
            "building output record batch from chunk-store rows",
        )
    })?;

    let output_batch = align_record_batch_to_schema(&batch, target_schema).map_err(|err| {
        ApiError::deserialization_with_source(None, err, "DataFusion schema mismatch error")
    })?;

    Ok(Some(output_batch))
}

impl<T: DataframeClientAPI> ExecutionPlan for SegmentStreamExec<T> {
    fn name(&self) -> &'static str {
        "SegmentStreamExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &Arc<PlanProperties> {
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
            chunk_info: self.chunk_info.clone(),
            query_expression: self.query_expression.clone(),
            projected_schema: self.projected_schema.clone(),
            target_partitions,
            client: self.client.clone(),
            pending_analytics: self.pending_analytics.clone(),
            metrics: Arc::clone(&self.metrics),
            plan_summary: self.plan_summary.clone(),
            captured_collectors: self.captured_collectors.clone(),
            snapshot_sent: Arc::clone(&self.snapshot_sent),
        };

        let partitioning = match &plan.props.as_ref().partitioning {
            Partitioning::RoundRobinBatch(_) => Partitioning::RoundRobinBatch(target_partitions),
            Partitioning::UnknownPartitioning(_) => {
                Partitioning::UnknownPartitioning(target_partitions)
            }
            Partitioning::Hash(expr, _) => Partitioning::Hash(expr.clone(), target_partitions),
        };
        plan.props = self
            .props
            .as_ref()
            .clone()
            .with_partitioning(partitioning)
            .into();

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
            pending_analytics: self.pending_analytics.clone(),
            captured_collectors: self.captured_collectors.clone(),
            snapshot_sent: Arc::clone(&self.snapshot_sent),
            metrics: Arc::clone(&self.metrics),
        };

        Ok(Box::pin(stream))
    }

    fn metrics(&self) -> Option<MetricsSet> {
        Some(build_metrics_set_for_explain(
            &self.metrics,
            self.target_partitions,
            self.pending_analytics.time_to_first_chunk(),
        ))
    }
}

impl<T: DataframeClientAPI> DisplayAs for SegmentStreamExec<T> {
    fn fmt_as(&self, t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SegmentStreamExec: num_partitions={:?}",
            self.target_partitions,
        )?;
        match t {
            DisplayFormatType::Default | DisplayFormatType::TreeRender => Ok(()),
            DisplayFormatType::Verbose => {
                let s = &self.plan_summary;
                write!(
                    f,
                    ", query_type={}, chunks={}, segments={}, bytes={}, \
                     filters_pushed_down={}, filters_applied_client_side={}, \
                     entity_path_narrowing={}",
                    s.query_type,
                    s.query_chunks,
                    s.query_segments,
                    re_format::format_bytes(s.query_bytes as f64),
                    s.filters_pushed_down,
                    s.filters_applied_client_side,
                    s.entity_path_narrowing_applied,
                )
            }
        }
    }
}
