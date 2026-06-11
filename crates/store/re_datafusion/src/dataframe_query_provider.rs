mod cpu_worker;
mod io_loop;

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};

use crate::analytics::{QueryErrorKind, build_metrics_set_for_explain};
use crate::chunk_fetcher::{batch_byte_size, batch_byte_size_uncompressed};
use crate::dataframe_query_common::{
    DataframeClientAPI, IndexValuesMap, PlanSummary, group_chunk_infos_by_segment_id,
};
use crate::metrics_capture::QueryMetrics;
use crate::pipeline_budget::PipelineBudget;
use arrow::array::RecordBatch;
use arrow::compute::SortOptions;
use arrow::datatypes::{Schema, SchemaRef};
use cpu_worker::{CpuWorkerMsg, chunk_store_cpu_worker_thread};
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
use datafusion::physical_plan::metrics::MetricsSet;
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use futures::FutureExt as _;
use futures_util::Stream;
use io_loop::chunk_stream_io_loop;
use itertools::Itertools as _;
use re_dataframe::{Index, QueryExpression, TimelineName};
use re_protos::cloud::v1alpha1::ScanSegmentTableResponse;
use re_redap_client::{ApiError, ApiResult};

use crate::IntoDfError as _;
use re_sorbet::{ColumnDescriptor, ColumnSelector};
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::Instrument as _;

/// This parameter sets the back pressure that either the streaming provider
/// can place on the CPU worker thread or the CPU worker thread can place on
/// the IO stream.
const CPU_THREAD_IO_CHANNEL_SIZE: usize = 32;

/// Helper to attach parent trace context if available.
/// Returns a guard that must be kept alive for the duration of the traced scope.
/// We can use this to ensure all phases of table provider's execution pipeline are
/// parented by a single trace.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
#[must_use]
pub(crate) fn attach_trace_context(
    trace_headers: Option<&crate::TraceHeaders>,
) -> Option<re_perf_telemetry::external::opentelemetry::ContextGuard> {
    trace_headers?.attach()
}

#[derive(Debug)]
pub(crate) struct SegmentStreamExec<T: DataframeClientAPI> {
    props: Arc<PlanProperties>,
    index_values: IndexValuesMap,

    /// Describes the chunks per partition, derived from `chunk_info_batches`.
    /// We keep both around so that we only have to process once, but we may
    /// reuse multiple times in theory. We may also need to recompute if the
    /// user asks for a different target partition. These are generally not
    /// too large.
    chunk_info: Arc<BTreeMap<String, Vec<RecordBatch>>>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
    target_partitions: usize,
    worker_runtime: Arc<CpuRuntime>,
    client: T,

    /// Optional row limit pushed down from the scan. When set, background
    /// threads will stop fetching/processing data once this many rows have
    /// been produced.
    limit_rows: Option<usize>,

    /// Request trace-headers.
    /// Passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    trace_headers: Option<crate::TraceHeaders>,

    /// Server-assigned response trace-id for this scan.
    /// This may or may not match request `trace_headers`.
    server_trace_id: Option<re_redap_client::TraceId>,

    /// Pending query analytics — always present; the OTLP send on drop is
    /// gated internally by whether the per-process telemetry stack is active.
    pending_analytics: crate::PendingQueryAnalytics,

    /// Subscribers (from `query_metrics()`) captured at plan-construction
    /// time. Each receives a [`crate::QuerySnapshot`] when the last
    /// per-partition stream completes — see [`DataframeSegmentStreamInner::maybe_emit_snapshot`].
    captured_collectors: Vec<crate::MetricsCollector>,

    /// Counts down to zero as per-partition streams complete; on the
    /// zero-transition the metrics snapshot is built and pushed to
    /// `captured_collectors`. Wrapped in an `Arc` so all streams + the plan
    /// share the same counter.
    partitions_remaining: Arc<AtomicUsize>,

    /// Latched true the first time the snapshot is sent so cancellation /
    /// late drop doesn't produce a duplicate.
    snapshot_sent: Arc<AtomicBool>,

    /// Shared byte budget for end-to-end pipeline backpressure.
    /// Created once and shared across all partitions so the total memory
    /// usage is bounded by a single global budget.
    pipeline_budget: Arc<PipelineBudget>,

    /// Per-query counters + embedded plan-time `QueryInfo`. Single source of
    /// truth for fetch counters: each IO task flushes its `TaskFetchStats`
    /// here, the snapshot path reads the atomics in `build_query_snapshot`,
    /// and `ExecutionPlan::metrics()` builds an ad-hoc `MetricsSet` from
    /// them on demand for `EXPLAIN ANALYZE`.
    metrics: Arc<QueryMetrics>,

    /// Plan-time summary used by `DisplayAs::Verbose` so that `EXPLAIN` (without
    /// `ANALYZE`) also exposes the most useful planning-phase decisions.
    plan_summary: PlanSummary,
}

pub struct DataframeSegmentStreamInner<T: DataframeClientAPI> {
    projected_schema: SchemaRef,
    client: T,
    chunk_infos: Vec<RecordBatch>,

    /// Name of the timeline named by `query_expression.filtered_index`,
    /// if any. Plumbed through to the IO loop so it can extract the
    /// per-chunk `{timeline}:start` values from the `chunk_info`
    /// columns and build per-segment manifests for the CPU worker's
    /// horizon-driven emit + GC. `TimelineName` is `Copy` (interned
    /// `Arc<str>`), so propagating it end-to-end avoids the
    /// String→&str→TimelineName conversion dance an owned `String`
    /// would force at each hop.
    filtered_index_timeline: Option<TimelineName>,

    chunk_tx: Option<Sender<ApiResult<CpuWorkerMsg>>>,
    store_output_channel: Receiver<RecordBatch>,
    io_join_handle: Option<JoinHandle<ApiResult<()>>>,

    /// We must keep a handle on the cpu runtime because the execution plan
    /// is dropped during streaming. We need this handle to continue to exist
    /// so that our worker does not shut down unexpectedly.
    #[expect(dead_code)]
    cpu_runtime: Arc<CpuRuntime>,
    cpu_join_handle: Option<JoinHandle<ApiResult<()>>>,

    /// Request trace-headers.
    /// Passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    trace_headers: Option<crate::TraceHeaders>,

    /// Server-assigned response trace-id for this scan.
    /// This may or may not match request `trace_headers`.
    server_trace_id: Option<re_redap_client::TraceId>,

    /// Pending query analytics — keeps alive until stream completes.
    pending_analytics: crate::PendingQueryAnalytics,

    /// Subscribers cloned from the parent `SegmentStreamExec`. On
    /// end-of-stream the last partition to finish builds a
    /// [`crate::QuerySnapshot`] and pushes it to each.
    captured_collectors: Vec<crate::MetricsCollector>,

    /// Shared partition countdown — see `SegmentStreamExec::partitions_remaining`.
    partitions_remaining: Arc<AtomicUsize>,

    /// Shared latch — see `SegmentStreamExec::snapshot_sent`.
    snapshot_sent: Arc<AtomicBool>,

    /// Shared metrics handle used by the snapshot path. Same `QueryMetrics`
    /// as the parent plan; reading the atomics gives the final fetch counters.
    metrics: Arc<QueryMetrics>,

    /// Shared byte budget for end-to-end pipeline backpressure.
    pipeline_budget: Arc<PipelineBudget>,
}

// TODO(RR-4607): This is a temporary fix to minimize the impact of leaking memory
// per issue <https://github.com/rerun-io/dataplatform/issues/1494>.
// The work around is to check for when the stream has exhausted and
// to set the `inner` to None, thereby clearing the memory since
// we are not properly getting a `drop` call from the upstream
// FFI interface. When the upstream issue resolves, change
// `DataframeSegmentStreamInner` back into `DataframeSegmentStream`
// and delete this wrapper struct.
pub struct DataframeSegmentStream<T: DataframeClientAPI> {
    inner: Option<DataframeSegmentStreamInner<T>>,
}

impl<T: DataframeClientAPI> Stream for DataframeSegmentStream<T> {
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this_outer = self.get_mut();
        let Some(this) = this_outer.inner.as_mut() else {
            return Poll::Ready(None);
        };

        #[cfg(not(target_arch = "wasm32"))]
        let _trace_guard = attach_trace_context(this.trace_headers.as_ref());

        // If we have any errors on the worker threads, we want to ensure we pass them up
        // through the stream. Any `ApiError` that didn't already carry a trace-id
        // picks up the scan's server trace-id on the way out.
        if let Some(join_handle) = this.cpu_join_handle.take_if(|h| h.is_finished())
            && let Some(cpu_join_result) = join_handle.now_or_never()
        {
            match cpu_join_result {
                Err(err) => {
                    this.pending_analytics.record_error(QueryErrorKind::Decode);
                    return Poll::Ready(Some(exec_err!("{err}")));
                }
                Ok(Err(err)) => {
                    this.pending_analytics.record_error(QueryErrorKind::Decode);
                    return Poll::Ready(Some(Err(err
                        .with_trace_id(this.server_trace_id)
                        .into_df_error())));
                }
                Ok(Ok(())) => {}
            }
        }

        // Also check the IO task — if it failed (e.g. gRPC 500 from FetchChunks),
        // the error would otherwise be silently lost because the CPU task sees a
        // closed channel and exits with Ok(()).
        if let Some(join_handle) = this.io_join_handle.take_if(|h| h.is_finished())
            && let Some(io_join_result) = join_handle.now_or_never()
        {
            match io_join_result {
                Err(err) => {
                    this.pending_analytics.record_error(QueryErrorKind::Other);
                    return Poll::Ready(Some(exec_err!("{err}")));
                }
                Ok(Err(err)) => {
                    // The IO task's own error-recording will have already set a
                    // more specific kind (direct_fetch / grpc_fetch) via OnceLock,
                    // so this call is a no-op fallback if that hasn't happened.
                    this.pending_analytics.record_error(QueryErrorKind::Other);
                    return Poll::Ready(Some(Err(err
                        .with_trace_id(this.server_trace_id)
                        .into_df_error())));
                }
                Ok(Ok(())) => {}
            }
        }

        // If this is the first call, we are uninitialized so create the io worker.
        // We check both `io_join_handle` (not yet spawned) and `chunk_tx` (not yet taken)
        // because the IO check above can consume `io_join_handle` after it finishes.
        if this.io_join_handle.is_none()
            && let Some(chunk_tx) = this.chunk_tx.take()
        {
            let io_handle = Handle::current();

            let client = this.client.clone();
            let chunk_infos = this.chunk_infos.clone();
            let filtered_index_timeline = this.filtered_index_timeline;
            let pending_analytics = this.pending_analytics.clone();
            let pipeline_budget = Arc::clone(&this.pipeline_budget);
            let metrics = Arc::clone(&this.metrics);

            // Parent the IO pipeline under the original caller's trace via the
            // `attach_trace_context` guard above, not under whichever DataFusion
            // driver thread happens to poll us first.
            let io_span = tracing::info_span!("chunk_io_pipeline");

            this.io_join_handle = Some(
                io_handle.spawn(
                    async move {
                        chunk_stream_io_loop(
                            client,
                            chunk_infos,
                            filtered_index_timeline,
                            chunk_tx,
                            pending_analytics,
                            pipeline_budget,
                            metrics,
                        )
                        .await
                    }
                    .instrument(io_span),
                ),
            );
        }

        let result = this
            .store_output_channel
            .poll_recv(cx)
            .map(|result| Ok(result).transpose());

        if matches!(&result, Poll::Ready(Some(Ok(_)))) {
            // This could be the first time we return data that will actually be
            // shown to the user — as close to the perceived latency as we'll get.
            // `record_first_chunk` is OnceLock-backed; subsequent calls are no-ops.
            this.pending_analytics.record_first_chunk();
        }

        if matches!(&result, Poll::Ready(None)) {
            // Eagerly produce a `QuerySnapshot` for `metrics_capture`
            // subscribers when this is the last per-partition stream to
            // finish — long before the FFI capsule on the Python side gets
            // garbage-collected and triggers `PendingInner::Drop`.
            this.maybe_emit_snapshot();
            this_outer.inner = None;
        }

        result
    }
}

impl<T: DataframeClientAPI> RecordBatchStream for DataframeSegmentStream<T> {
    fn schema(&self) -> SchemaRef {
        self.inner
            .as_ref()
            .map(|inner| inner.projected_schema.clone())
            .unwrap_or_else(|| Schema::empty().into())
    }
}

/// Build a [`crate::QuerySnapshot`] from the canonical sources and push it to
/// each subscriber. Caller is responsible for the CAS on `snapshot_sent` —
/// this function is the pure "actually build + send" step.
fn build_and_push_snapshot(
    captured_collectors: &[crate::MetricsCollector],
    pending_analytics: &crate::PendingQueryAnalytics,
    metrics: &QueryMetrics,
) {
    let snapshot = crate::metrics_capture::build_query_snapshot(
        metrics,
        pending_analytics.total_duration(),
        pending_analytics.time_to_first_chunk(),
        pending_analytics.error_kind(),
        pending_analytics.direct_terminal_reason(),
    );
    crate::metrics_capture::push_snapshot(captured_collectors, &snapshot);
}

/// Decrement `partitions_remaining` and, on the transition to zero, latch
/// `snapshot_sent` and emit a snapshot to each subscriber. Idempotent —
/// the CAS on `snapshot_sent` guarantees at most one emission across all
/// callers (`poll_next` end-of-stream, the empty-partition branch in
/// `execute`, and concurrent cancellations).
fn emit_snapshot_if_last_partition(
    captured_collectors: &[crate::MetricsCollector],
    partitions_remaining: &AtomicUsize,
    snapshot_sent: &AtomicBool,
    pending_analytics: &crate::PendingQueryAnalytics,
    metrics: &QueryMetrics,
) {
    if captured_collectors.is_empty() {
        // Fast path: no `query_metrics()` scope was active when this plan was
        // built, so no snapshot will ever be emitted. The counter only exists
        // to detect the zero-transition for that emission, so skip the RMW —
        // it would just bounce the cache line across partitions for nothing.
        return;
    }

    let prev = partitions_remaining.fetch_sub(1, Ordering::AcqRel);
    if prev != 1 {
        return; // not the last partition yet
    }

    if snapshot_sent
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    build_and_push_snapshot(captured_collectors, pending_analytics, metrics);
}

/// Cancellation/short-circuit fallback: if no other path has emitted yet,
/// latch `snapshot_sent` and emit with whatever state has been recorded.
/// Does NOT touch `partitions_remaining` (the normal end-of-stream path
/// is responsible for that).
fn emit_snapshot_drop_fallback(
    captured_collectors: &[crate::MetricsCollector],
    snapshot_sent: &AtomicBool,
    pending_analytics: &crate::PendingQueryAnalytics,
    metrics: &QueryMetrics,
) {
    if captured_collectors.is_empty() {
        return;
    }
    if snapshot_sent
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    build_and_push_snapshot(captured_collectors, pending_analytics, metrics);
}

impl<T: DataframeClientAPI> DataframeSegmentStreamInner<T> {
    /// See [`emit_snapshot_if_last_partition`].
    fn maybe_emit_snapshot(&self) {
        emit_snapshot_if_last_partition(
            &self.captured_collectors,
            &self.partitions_remaining,
            &self.snapshot_sent,
            &self.pending_analytics,
            &self.metrics,
        );
    }
}

impl<T: DataframeClientAPI> Drop for DataframeSegmentStreamInner<T> {
    fn drop(&mut self) {
        // Cover the cancelled/short-circuited case — `poll_next` may already
        // have called `maybe_emit_snapshot`, in which case the CAS no-ops.
        emit_snapshot_drop_fallback(
            &self.captured_collectors,
            &self.snapshot_sent,
            &self.pending_analytics,
            &self.metrics,
        );
    }
}

impl<T: DataframeClientAPI> SegmentStreamExec<T> {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        num_partitions: usize,
        chunk_info_batches: Option<RecordBatch>,
        mut query_expression: QueryExpression,
        index_values: IndexValuesMap,
        client: T,
        limit_rows: Option<usize>,
        trace_headers: Option<crate::TraceHeaders>,
        server_trace_id: Option<re_redap_client::TraceId>,
        pending_analytics: crate::PendingQueryAnalytics,
        metrics: Arc<QueryMetrics>,
        captured_collectors: Vec<crate::MetricsCollector>,
    ) -> datafusion::common::Result<Self> {
        let projected_schema = match projection {
            Some(p) => Arc::new(table_schema.project(p)?),
            None => Arc::clone(table_schema),
        };

        if projection.is_some_and(|projection| !projection.is_empty()) {
            let selection = projected_schema
                .fields()
                .iter()
                .map(|field| {
                    ColumnDescriptor::try_from_arrow_field(None, field).map(ColumnSelector::from)
                })
                .try_collect()
                .map_err(|err| exec_datafusion_err!("{err}"))?;

            query_expression.selection = Some(selection);
        }

        // The output ordering of this table provider should always be rerun
        // segment ID and then time index. If the output does not have rerun
        // segment ID included, we cannot specify any output ordering.

        let orderings = if projected_schema
            .fields()
            .iter()
            .any(|f| f.name().as_str() == ScanSegmentTableResponse::FIELD_SEGMENT_ID)
        {
            let segment_col = Arc::new(Column::new(ScanSegmentTableResponse::FIELD_SEGMENT_ID, 0))
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
                segment_col,
                SortOptions::new(false, true),
            )];
            if let Some(col_expr) = order_col {
                physical_ordering.push(PhysicalSortExpr::new(
                    col_expr,
                    SortOptions::new(false, true),
                ));
            }
            vec![
                LexOrdering::new(physical_ordering)
                    .expect("LexOrdering should return Some since input is not empty"),
            ]
        } else {
            vec![]
        };

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

        // Compute total uncompressed size for adaptive budget before consuming the batches.
        let total_uncompressed: usize = chunk_info_batches
            .iter()
            .map(|b| batch_byte_size_uncompressed(b).unwrap_or_else(|| batch_byte_size(b)))
            .sum::<u64>() as usize;

        let chunk_info = group_chunk_infos_by_segment_id(chunk_info_batches.as_slice())?;
        drop(chunk_info_batches);

        let worker_runtime = Arc::new(CpuRuntime::try_new(num_partitions)?);

        // `metrics` carries the plan-time `QueryInfo` (set by the caller in
        // `dataframe_query_common::scan`) plus zero-initialized fetch atomics
        // that the IO loop will populate.
        let plan_summary = PlanSummary::from_query_info(&metrics.query_info);

        // `captured_collectors` was sourced from the `DataframeQueryTableProvider`
        // (which itself read the Python `query_metrics()` ContextVar at the
        // `dataset_view.rs::reader()` boundary). It is empty when no scope
        // was active.
        let partitions_remaining = Arc::new(AtomicUsize::new(num_partitions));
        let snapshot_sent = Arc::new(AtomicBool::new(false));

        Ok(Self {
            props,
            chunk_info,
            query_expression,
            index_values,
            projected_schema,
            target_partitions: num_partitions,
            worker_runtime,
            client,
            limit_rows,
            trace_headers,
            server_trace_id,
            pending_analytics,
            pipeline_budget: Arc::new(PipelineBudget::new(total_uncompressed, num_partitions)),
            metrics,
            plan_summary,
            captured_collectors,
            partitions_remaining,
            snapshot_sent,
        })
    }
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

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        re_tracing::profile_function!();

        // Attach the remote parent context so any spans created below
        // (`cpu_worker`, and `chunk_io_pipeline` later in `poll_next`) inherit
        // the caller's trace.
        //
        // We deliberately do NOT open an `execute` tracing span here: `execute`
        // returns synchronously after spawning the workers (~µs), so an
        // `execute` span would be a misleading tiny leaf with the heavy
        // long-running children (`cpu_worker`, `chunk_io_pipeline`) hidden
        // beneath it. Parent those workers directly under the caller's trace
        // instead.
        #[cfg(not(target_arch = "wasm32"))]
        let _trace_guard = attach_trace_context(self.trace_headers.as_ref());

        let pipeline_budget = Arc::clone(&self.pipeline_budget);

        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);

        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let chunk_infos: Vec<_> = {
            re_tracing::profile_scope!("concat_chunk_infos_per_segment");
            self.chunk_info
                .iter()
                .filter(|(segment_id, _)| {
                    let hash_value = segment_id.hash_one(&random_state) as usize;
                    hash_value % self.target_partitions == partition
                })
                // Drop segments not referenced by `index_values` before
                // they reach the IO loop. Otherwise the IO loop would
                // fetch + `PipelineBudget::commit` their chunks, then the
                // CPU worker would drop them on an `index_values` miss
                // without releasing — leaking the reservation for the
                // rest of the query.
                .filter(|(segment_id, _)| {
                    self.index_values.as_ref().is_none_or(|iv| {
                        iv.contains_key(&re_types_core::SegmentId::from(segment_id.as_str()))
                    })
                })
                // we end up with 1 batch per (rerun) segment. Order is important and must be preserved.
                // See SegmentStreamExec::try_new for details on ordering.
                .map(|(_, batches)| re_arrow_util::concat_polymorphic_batches(batches))
                .try_collect()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        None,
                        err,
                        "concatenating chunk-info batches per segment",
                    )
                    .into_df_error()
                })?
        };

        // if no chunks match this datafusion partition, return an empty stream
        if chunk_infos.is_empty() {
            // The partition still counts toward `partitions_remaining` —
            // decrement it (and possibly emit the snapshot) here, since the
            // empty stream's `poll_next` never gets the chance.
            emit_snapshot_if_last_partition(
                &self.captured_collectors,
                &self.partitions_remaining,
                &self.snapshot_sent,
                &self.pending_analytics,
                &self.metrics,
            );
            let stream: DataframeSegmentStream<T> = DataframeSegmentStream { inner: None };
            return Ok(Box::pin(stream));
        }

        let client = self.client.clone();

        let (batches_tx, batches_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);
        let query_expression = self.query_expression.clone();
        let projected_schema = self.projected_schema.clone();
        let limit_rows = self.limit_rows;
        let cpu_join_handle = Some(
            self.worker_runtime.handle().spawn(
                chunk_store_cpu_worker_thread(
                    chunk_rx,
                    batches_tx,
                    query_expression,
                    projected_schema,
                    self.index_values.clone(),
                    limit_rows,
                    Arc::clone(&pipeline_budget),
                )
                .instrument(tracing::info_span!("cpu_worker")),
            ),
        );

        let filtered_index_timeline = self.query_expression.filtered_index;

        let stream = DataframeSegmentStreamInner {
            projected_schema: self.projected_schema.clone(),
            store_output_channel: batches_rx,
            client,
            chunk_infos,
            filtered_index_timeline,
            chunk_tx: Some(chunk_tx),
            io_join_handle: None,
            cpu_join_handle,
            cpu_runtime: Arc::clone(&self.worker_runtime),
            trace_headers: self.trace_headers.clone(),
            server_trace_id: self.server_trace_id,
            pending_analytics: self.pending_analytics.clone(),
            pipeline_budget,
            captured_collectors: self.captured_collectors.clone(),
            partitions_remaining: Arc::clone(&self.partitions_remaining),
            snapshot_sent: Arc::clone(&self.snapshot_sent),
            metrics: Arc::clone(&self.metrics),
        };
        let stream = DataframeSegmentStream {
            inner: Some(stream),
        };

        Ok(Box::pin(stream))
    }

    fn metrics(&self) -> Option<MetricsSet> {
        // Build the `MetricsSet` on demand from our flat `QueryMetrics` —
        // see `build_metrics_set_for_explain` for why we don't hold a
        // `MetricsSet` as the source of truth.
        Some(build_metrics_set_for_explain(
            &self.metrics,
            self.target_partitions,
            self.pending_analytics.time_to_first_chunk(),
        ))
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
            index_values: self.index_values.clone(),
            projected_schema: self.projected_schema.clone(),
            target_partitions,
            worker_runtime: Arc::new(CpuRuntime::try_new(target_partitions)?),
            client: self.client.clone(),
            limit_rows: self.limit_rows,
            trace_headers: self.trace_headers.clone(),
            server_trace_id: self.server_trace_id,
            pending_analytics: self.pending_analytics.clone(),
            pipeline_budget: Arc::clone(&self.pipeline_budget),
            // Share the same `QueryMetrics` atomics across the original and
            // repartitioned plans so any in-flight writes are still observed
            // by the original `PendingInner::drop` snapshot path.
            metrics: Arc::clone(&self.metrics),
            plan_summary: self.plan_summary.clone(),
            captured_collectors: self.captured_collectors.clone(),
            // Reset the partition counter for the repartitioned plan — it
            // owns its own per-partition stream lifetimes.
            partitions_remaining: Arc::new(AtomicUsize::new(target_partitions)),
            // Repartitioning yields a fresh plan; the snapshot latch is
            // shared so the original plan's `Drop`-path emission still
            // dedupes against the new one.
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
                    re_format::format_bytes(s.query_bytes as _),
                    s.filters_pushed_down,
                    s.filters_applied_client_side,
                    s.entity_path_narrowing_applied,
                )
            }
        }
    }
}

#[derive(Debug)]
struct CpuRuntime {
    /// Handle is the tokio structure for interacting with a Runtime.
    handle: Handle,

    /// Signal to start shutting down
    notify_shutdown: Arc<Notify>,

    /// When thread is active, is Some
    thread_join_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CpuRuntime {
    fn drop(&mut self) {
        // Notify the thread to shut down.
        self.notify_shutdown.notify_one();
        if let Some(thread_join_handle) = self.thread_join_handle.take() {
            // If the thread is still running, we wait for it to finish
            if thread_join_handle.join().is_err() {
                re_log::error!("Error joining CPU runtime thread");
            }
        }
    }
}

impl CpuRuntime {
    /// Create a new Tokio Runtime for CPU bound tasks
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn try_new(num_threads: usize) -> Result<Self, DataFusionError> {
        let cpu_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_threads)
            .build()?;
        let handle = cpu_runtime.handle().clone();
        let notify_shutdown = Arc::new(Notify::new());
        let notify_shutdown_captured: Arc<Notify> = Arc::clone(&notify_shutdown);

        // The cpu_runtime runs and is dropped on a separate thread

        let thread_join_handle = std::thread::Builder::new()
            .name("datafusion_query_cpu_thread".to_owned())
            .spawn(move || {
                cpu_runtime.block_on(async move {
                    notify_shutdown_captured.notified().await;
                });
                // Note: cpu_runtime is dropped here, which will wait for all tasks
                // to complete
            })?;

        Ok(Self {
            handle,
            notify_shutdown,
            thread_join_handle: Some(thread_join_handle),
        })
    }

    /// Return a handle suitable for spawning CPU bound tasks
    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}
