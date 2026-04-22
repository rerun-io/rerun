use std::any::Any;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll};

use crate::chunk_fetcher::{
    SortedChunksWithSegment, batch_byte_size, batch_has_any_direct_urls, fetch_batch_direct,
    fetch_batch_group_via_grpc, split_batch_by_direct_url,
};
use crate::dataframe_query_common::{
    DataframeClientAPI, IndexValuesMap, group_chunk_infos_by_segment_id,
    prepend_string_column_schema,
};
use arrow::array::{Array, RecordBatch, RecordBatchOptions, StringArray, UInt64Array};
use arrow::compute::SortOptions;
use arrow::datatypes::{Schema, SchemaRef};
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
use futures::StreamExt as _;
use futures_util::{FutureExt as _, Stream};
use re_dataframe::external::re_chunk::Chunk;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{
    ChunkStoreHandle, Index, QueryCache, QueryEngine, QueryExpression, QueryHandle, StorageEngine,
};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ScanSegmentTableResponse;
use re_redap_client::{ApiError, ApiResult};

use crate::IntoDfError as _;
use re_sorbet::{ColumnDescriptor, ColumnSelector};
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::Instrument as _;

// TODO(zehiko) make these configurable

/// This parameter sets the back pressure that either the streaming provider
/// can place on the CPU worker thread or the CPU worker thread can place on
/// the IO stream.
const CPU_THREAD_IO_CHANNEL_SIZE: usize = 32;

/// Target batch size in bytes for grouping segments together in requests.
/// This reduces the number of round-trips while keeping memory usage bounded (as long
/// as the concurrency is also bounded).
const TARGET_BATCH_SIZE_BYTES: usize = 8 * 1024 * 1024; // 8 MB

/// How many concurrent requests to make to the server when fetching chunks.
const GRPC_BATCH_SIZE: usize = 12;

/// Max batch-level futures in-flight at once in the IO pipeline.
/// This bounds both concurrency and the reorder buffer size.
const IO_PIPELINE_BUFFER: usize = 24;

/// Environment variable to force the client to go through the `FetchChunks` data fetching path.
static CHUNK_STRATEGY: LazyLock<String> = LazyLock::new(|| {
    std::env::var("RERUN_CHUNK_STRATEGY")
        .unwrap_or_default()
        .to_ascii_lowercase()
});

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
    props: PlanProperties,
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
    limit: Option<usize>,

    /// Request trace-headers.
    /// Passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    trace_headers: Option<crate::TraceHeaders>,

    /// Server-assigned response trace-id for this scan.
    /// This may or may not match request `trace_headers`.
    server_trace_id: Option<re_redap_client::TraceId>,

    /// Pending query analytics — fetch stats are accumulated here.
    /// The event is sent when the last clone is dropped.
    pending_analytics: Option<crate::PendingQueryAnalytics>,
}

use crate::chunk_fetcher::ChunksWithSegment;

pub struct DataframeSegmentStreamInner<T: DataframeClientAPI> {
    projected_schema: SchemaRef,
    client: T,
    chunk_infos: Vec<RecordBatch>,

    chunk_tx: Option<Sender<ApiResult<SortedChunksWithSegment>>>,
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
    pending_analytics: Option<crate::PendingQueryAnalytics>,
}

/// This is a temporary fix to minimize the impact of leaking memory
/// per issue <https://github.com/rerun-io/dataplatform/issues/1494>
/// The work around is to check for when the stream has exhausted and
/// to set the `inner` to None, thereby clearing the memory since
/// we are not properly getting a `drop` call from the upstream
/// FFI interface. When the upstream issue resolves, change
/// `DataframeSegmentStreamInner` back into `DataframeSegmentStream`
/// and delete this wrapper struct.
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
                Err(err) => return Poll::Ready(Some(exec_err!("{err}"))),
                Ok(Err(err)) => {
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
                Err(err) => return Poll::Ready(Some(exec_err!("{err}"))),
                Ok(Err(err)) => {
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
            let pending_analytics = this.pending_analytics.clone();

            let io_span = tracing::info_span!("chunk_io_pipeline");

            this.io_join_handle = Some(
                io_handle.spawn(
                    async move {
                        chunk_stream_io_loop(client, chunk_infos, chunk_tx, pending_analytics).await
                    }
                    .instrument(io_span),
                ),
            );
        }

        let result = this
            .store_output_channel
            .poll_recv(cx)
            .map(|result| Ok(result).transpose());

        if matches!(&result, Poll::Ready(Some(Ok(_))))
            && let Some(analytics) = &this.pending_analytics
        {
            // This could be the first time we return data that will
            // actually be shown to the user.
            // This is as close to the perceived latency as we're gonna come right now.
            analytics.record_first_chunk();
        }

        if matches!(&result, Poll::Ready(None)) {
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

impl<T: DataframeClientAPI> SegmentStreamExec<T> {
    #[tracing::instrument(level = "info", skip_all)]
    #[expect(clippy::too_many_arguments)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        num_partitions: usize,
        chunk_info_batches: Option<RecordBatch>,
        mut query_expression: QueryExpression,
        index_values: IndexValuesMap,
        client: T,
        limit: Option<usize>,
        trace_headers: Option<crate::TraceHeaders>,
        server_trace_id: Option<re_redap_client::TraceId>,
        pending_analytics: Option<crate::PendingQueryAnalytics>,
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
                .collect::<Result<Vec<_>, _>>()
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
        );

        let chunk_info = group_chunk_infos_by_segment_id(chunk_info_batches.as_slice())?;
        drop(chunk_info_batches);

        let worker_runtime = Arc::new(CpuRuntime::try_new(num_partitions)?);

        Ok(Self {
            props,
            chunk_info,
            query_expression,
            index_values,
            projected_schema,
            target_partitions: num_partitions,
            worker_runtime,
            client,
            limit,
            trace_headers,
            server_trace_id,
            pending_analytics,
        })
    }
}

#[tracing::instrument(level = "trace", skip_all)]
async fn send_next_row(
    query_handle: &QueryHandle<StorageEngine>,
    segment_id: &str,
    target_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit: Option<usize>,
) -> ApiResult<Option<()>> {
    // If we have already sent enough rows, stop early.
    if limit.is_some_and(|l| *rows_sent >= l) {
        return Ok(None);
    }

    let query_schema = Arc::clone(query_handle.schema());
    let num_fields = query_schema.fields.len();

    let Some(mut next_row) = query_handle.next_row() else {
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

    next_row.insert(0, sid_array);

    let batch_schema = Arc::new(prepend_string_column_schema(
        &query_schema,
        ScanSegmentTableResponse::FIELD_SEGMENT_ID,
    ));

    let batch = RecordBatch::try_new_with_options(
        batch_schema,
        next_row,
        &RecordBatchOptions::default().with_row_count(Some(num_rows)),
    )
    .map_err(|err| {
        ApiError::deserialization_with_source(
            None,
            err,
            "building output record batch from chunk-store rows",
        )
    })?;

    // align the batch to the target schema, this should be always possible
    // by construction.
    let output_batch = align_record_batch_to_schema(&batch, target_schema).map_err(|err| {
        ApiError::internal_with_source(None, err, "DataFusion schema mismatch error")
    })?;

    // Slice the batch to respect the row limit
    let output_batch = if let Some(limit) = limit {
        let remaining = limit.saturating_sub(*rows_sent);
        if remaining == 0 {
            return Ok(None);
        }
        if output_batch.num_rows() > remaining {
            output_batch.slice(0, remaining)
        } else {
            output_batch
        }
    } else {
        output_batch
    };

    *rows_sent += output_batch.num_rows();

    output_channel
        .send(output_batch)
        .await
        .map_err(|err| ApiError::internal_with_source(None, err, "output channel closed"))?;

    Ok(Some(()))
}

// TODO(#10781) - support for sending intermediate results/chunks
async fn chunk_store_cpu_worker_thread(
    mut input_channel: Receiver<ApiResult<SortedChunksWithSegment>>,
    output_channel: Sender<RecordBatch>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
    index_values: IndexValuesMap,
    limit: Option<usize>,
) -> ApiResult<()> {
    struct CurrentStores {
        segment_id: String,
        store: ChunkStoreHandle,
        query_handle: QueryHandle<StorageEngine>,
    }

    impl CurrentStores {
        fn new(
            segment_id: String,
            query_expression: &QueryExpression,
            index_values: &IndexValuesMap,
        ) -> Self {
            let store_id = StoreId::random(
                StoreKind::Recording,
                ApplicationId::from(segment_id.as_str()),
            );
            let store = ChunkStore::new_handle(store_id.clone(), Default::default());

            let query_engine =
                QueryEngine::new(store.clone(), QueryCache::new_handle(store.clone()));
            let mut individual_query = query_expression.clone();

            let values = index_values
                .as_ref()
                .and_then(|index_values| index_values.get(&segment_id));
            if let Some(values) = values {
                individual_query.using_index_values = Some(values.clone());
            }

            let query_handle = query_engine.query(individual_query);

            Self {
                segment_id,
                store,
                query_handle,
            }
        }

        /// Flush all remaining rows from the query handle, respecting the row limit.
        async fn flush(
            self,
            projected_schema: &Arc<Schema>,
            output_channel: &Sender<RecordBatch>,
            rows_sent: &mut usize,
            limit: Option<usize>,
        ) -> ApiResult<()> {
            while send_next_row(
                &self.query_handle,
                &self.segment_id,
                projected_schema,
                output_channel,
                rows_sent,
                limit,
            )
            .await?
            .is_some()
            {}
            Ok(())
        }
    }
    let mut current_stores: Option<CurrentStores> = None;
    let mut rows_sent: usize = 0;
    while let Some(chunks_and_segment_ids) = input_channel.recv().await {
        let (segment_id, chunks) = chunks_and_segment_ids?;

        if chunks.is_empty() {
            continue;
        }

        if index_values
            .as_ref()
            .is_some_and(|index_values| !index_values.contains_key(&segment_id))
        {
            continue;
        }

        // When we change segments, flush the outputs
        if let Some(current_stores) = current_stores.take_if(|s| s.segment_id != segment_id) {
            current_stores
                .flush(&projected_schema, &output_channel, &mut rows_sent, limit)
                .await?;

            if limit.is_some_and(|l| rows_sent >= l) {
                return Ok(());
            }
        }

        let CurrentStores { store, .. } = current_stores.get_or_insert_with(|| {
            CurrentStores::new(segment_id, &query_expression, &index_values)
        });

        for chunk in chunks {
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

    // Flush out remaining of last segment
    if let Some(current_stores) = current_stores {
        current_stores
            .flush(&projected_schema, &output_channel, &mut rows_sent, limit)
            .await?;
    }

    Ok(())
}

/// Extract segment ID from a `chunk_info` `RecordBatch`. Each `chunk_info` batch contains
/// chunks *for a single segment*, hence we can just take the first row's `segment_id`. This is
/// guaranteed by the implementation in `group_chunk_infos_by_segment_id`.
fn extract_segment_id(chunk_info: &RecordBatch) -> ApiResult<String> {
    let segment_ids = chunk_info
        .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID)
        .ok_or_else(|| ApiError::internal("missing segment_id column in chunk_info batch"))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ApiError::internal("segment_id column is not a string array"))?;

    Ok(segment_ids.value(0).to_owned())
}

/// Extract chunk sizes from a `chunk_info` `RecordBatch`.
/// Returns a reference to the arrow array containing `chunk_byte_len` values.
fn extract_chunk_sizes(chunk_info: &RecordBatch) -> ApiResult<&UInt64Array> {
    let chunk_sizes = chunk_info
        .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)
        .ok_or_else(|| ApiError::internal("missing chunk_byte_len column in chunk_info batch"))?
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| ApiError::internal("chunk_byte_len column is not a uint64 array"))?;

    Ok(chunk_sizes)
}

type BatchingResult = (Vec<RecordBatch>, Vec<String>);

/// Groups `chunk_infos` into batches targeting the specified size, with special handling
/// for segments larger than the target size (which get split). Batches smaller than `target_size`
/// are merged together to reduce the number of requests.
///
/// Returns (batches, `segment_order`) where:
/// - batches: list of merged `RecordBatch`es, each representing a `target_size` request
/// - `segment_order`: Original order of segments for preserving segment order
fn create_request_batches(
    chunk_infos: Vec<RecordBatch>,
    target_size_bytes: u64,
) -> ApiResult<BatchingResult> {
    let merge_err = |err: arrow::error::ArrowError, ctx: &'static str| {
        ApiError::deserialization_with_source(None, err, ctx)
    };

    let mut request_batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_batch_size = 0u64;
    let mut segment_order = Vec::new();
    let mut segment_seen = HashSet::new();

    for chunk_info in chunk_infos {
        let segment_id = extract_segment_id(&chunk_info)?;
        let chunk_sizes = extract_chunk_sizes(&chunk_info)?;
        let segment_size: u64 = chunk_sizes.iter().map(|v| v.unwrap_or(0)).sum();

        // Track original segment order
        if segment_seen.insert(segment_id.clone()) {
            segment_order.push(segment_id.clone());
        }

        // Check if this segment would make the current batch too large
        if !current_batch.is_empty() && current_batch_size + segment_size > target_size_bytes {
            // Merge current batch and add to results
            let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
            request_batches.push(merged_batch);
            current_batch = Vec::new();
            current_batch_size = 0;
        }

        // Split the large segment into multiple requests
        if segment_size > target_size_bytes {
            // If current batch is not empty, merge and send it first
            if !current_batch.is_empty() {
                let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                    .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
                request_batches.push(merged_batch);
                current_batch = Vec::new();
                current_batch_size = 0;
            }

            let split_batches =
                split_large_segments(&segment_id, &chunk_info, target_size_bytes, chunk_sizes)?;

            // Split batches are already individual RecordBatches, add them directly
            for split_batch in split_batches {
                request_batches.push(split_batch);
            }
        } else {
            current_batch.push(chunk_info);
            current_batch_size += segment_size;
        }
    }

    // Don't forget to merge the last batch
    if !current_batch.is_empty() {
        let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
            .map_err(|err| merge_err(err, "merging final chunk-info batch"))?;
        request_batches.push(merged_batch);
    }

    tracing::debug!(
        "Batching complete: {} segments → {} batches (target_size={}KB)",
        segment_order.len(),
        request_batches.len(),
        target_size_bytes / 1024
    );

    Ok((request_batches, segment_order))
}

/// Split segment larger than target size into multiple smaller requests. Each request will contain
/// a subset of the chunks from the original segment, targeting approximately the desired size.
fn split_large_segments(
    segment_id: &str,
    chunk_info: &RecordBatch,
    target_size: u64,
    chunk_sizes: &UInt64Array,
) -> ApiResult<Vec<RecordBatch>> {
    let take_err = |err: arrow::error::ArrowError| {
        ApiError::deserialization_with_source(None, err, "slicing large segment into sub-batches")
    };

    let mut result_batches = Vec::new();
    let mut current_indices = Vec::new();
    let mut current_size = 0u64;

    for row_idx in 0..chunk_info.num_rows() {
        let chunk_size = chunk_sizes.value(row_idx);

        // Always include at least one chunk per batch (even if it exceeds target)
        if current_indices.is_empty() || current_size + chunk_size <= target_size {
            current_indices.push(row_idx);
            current_size += chunk_size;
        } else {
            // Create batch from current indices
            let batch =
                re_arrow_util::take_record_batch(chunk_info, &current_indices).map_err(take_err)?;
            result_batches.push(batch);

            // Start new batch with current chunk
            current_indices = vec![row_idx];
            current_size = chunk_size;
        }
    }

    // Don't forget the last batch
    if !current_indices.is_empty() {
        let batch =
            re_arrow_util::take_record_batch(chunk_info, &current_indices).map_err(take_err)?;
        result_batches.push(batch);
    }

    tracing::debug!(
        "Split large segment '{}' ({} bytes) into {} requests",
        segment_id,
        (0..chunk_info.num_rows())
            .map(|i| chunk_sizes.value(i))
            .sum::<u64>(),
        result_batches.len()
    );

    Ok(result_batches)
}

/// Helper function to sort chunks by segment order.
/// This function handles the fact we send concurrent requests where sometimes even a
/// single request can contain chunks from multiple segments (due to batching) and the more
/// important fact that server provides no ordering guarantees.
fn sort_chunks_by_segment_order(
    chunks: Vec<ChunksWithSegment>,
    segment_order: &[String],
) -> Vec<SortedChunksWithSegment> {
    use std::collections::HashMap;

    // Collect all individual chunks grouped by segment ID (we don't care about ordering of individual
    // chunks within a segment here)
    let mut segment_groups: HashMap<String, Vec<Chunk>> = HashMap::default();

    // Extract all chunks and group by segment
    for chunks_with_segment in chunks {
        for (chunk, segment_id_opt) in chunks_with_segment {
            let Some(segment_id) = segment_id_opt else {
                continue;
            };
            segment_groups.entry(segment_id).or_default().push(chunk);
        }
    }

    // Rebuild chunks in the correct segment order
    segment_order
        .iter()
        .filter_map(|segment_id| segment_groups.remove_entry(segment_id))
        .collect()
}

/// Helper to sort and send chunks through the output channel, preserving segment order.
/// Returns `false` if the output channel is closed (consumer dropped).
async fn send_sorted_chunks(
    chunks: Vec<ChunksWithSegment>,
    global_segment_order: &[String],
    output_channel: &Sender<ApiResult<SortedChunksWithSegment>>,
) -> bool {
    let sorted = {
        let _span = tracing::info_span!("sort_chunks").entered();
        sort_chunks_by_segment_order(chunks, global_segment_order)
    };
    let n_sorted = sorted.len();
    async {
        for chunk in sorted {
            if output_channel.send(Ok(chunk)).await.is_err() {
                return false;
            }
        }
        true
    }
    .instrument(tracing::info_span!("send_chunks", n = n_sorted))
    .await
}

/// Fetch remaining batches via batched gRPC (groups of `GRPC_BATCH_SIZE`),
/// preserving ordering.
async fn fetch_remaining_via_grpc<T: DataframeClientAPI>(
    batches: &[RecordBatch],
    client: &T,
    global_segment_order: &[String],
    output_channel: &Sender<ApiResult<SortedChunksWithSegment>>,
) -> ApiResult<()> {
    for batch_group in batches.chunks(GRPC_BATCH_SIZE) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes: u64 = batch_group.iter().map(batch_byte_size).sum();
            crate::chunk_fetcher::metrics::record_grpc_no_direct_urls(bytes);
        }
        let all_chunks = fetch_batch_group_via_grpc(batch_group, client).await?;
        if !send_sorted_chunks(all_chunks, global_segment_order, output_channel).await {
            return Ok(());
        }
    }
    Ok(())
}

/// This is the function that will run on the IO (main) tokio runtime that will listen
/// to the gRPC channel for chunks coming in from the Data Platform. This loop is started
/// up by the execute fn of the physical plan, so we will start one per output DataFusion partition,
/// which is different from the Rerun `segment_id`. The sorting by time index will happen within
/// the cpu worker thread.
///
/// `chunk_infos` is a list of batches with chunk information where each batch has info for
/// a *single segment*. We also expect these to be previously sorted by segment id, otherwise
/// our suggestion to the query planner that inputs are sorted by segment id will be incorrect.
/// See `group_chunk_infos_by_segment_id` and `execute` for more details.
///
/// In order to improve performance, while maintaining ordering, we batch requests to the server
/// and process them concurrently in groups. After data for each group is collected, it is sorted
/// by the input segment order before being sent to the CPU worker thread.
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(n_chunks, n_batches, n_segments, fetch_strategy,)
)]
async fn chunk_stream_io_loop<T: DataframeClientAPI>(
    client: T,
    chunk_infos: Vec<RecordBatch>,
    output_channel: Sender<ApiResult<SortedChunksWithSegment>>,
    pending_analytics: Option<crate::PendingQueryAnalytics>,
) -> ApiResult<()> {
    let target_size_bytes = TARGET_BATCH_SIZE_BYTES as u64;

    let n_chunks = chunk_infos.len();
    let (request_batches, global_segment_order) =
        create_request_batches(chunk_infos, target_size_bytes)?;

    let span = tracing::Span::current();
    span.record("n_chunks", n_chunks);
    span.record("n_batches", request_batches.len());
    span.record("n_segments", global_segment_order.len());

    re_log::debug!(
        "Fetching {n_chunks} chunks in {} batches ({} segments)",
        request_batches.len(),
        global_segment_order.len()
    );

    // Allow overriding the fetch strategy via environment variable.
    let force_grpc = *CHUNK_STRATEGY == "grpc";

    // Fast path: if no batches contain direct URLs (or gRPC is forced), fetch everything via gRPC.
    if force_grpc || !request_batches.iter().any(batch_has_any_direct_urls) {
        let reason = if force_grpc {
            "grpc_forced"
        } else {
            "no_direct_urls"
        };
        span.record("fetch_strategy", reason);
        re_log::debug!(
            "{reason}, fetching all {} chunks via FetchChunks gRPC",
            request_batches.len()
        );
        let result = fetch_remaining_via_grpc(
            &request_batches,
            &client,
            &global_segment_order,
            &output_channel,
        )
        .await;

        // All fetches were gRPC — record total bytes
        if let Some(analytics) = &pending_analytics {
            let total_bytes: u64 = request_batches.iter().map(batch_byte_size).sum();
            analytics.record_grpc_fetch(total_bytes);
        }

        return result;
    }

    // Split each batch into direct-URL rows and non-URL rows, producing independent work items.
    // Each work item gets a sequential index for the reorder buffer.
    enum FetchTask {
        Direct(RecordBatch),
        Grpc(RecordBatch),
    }

    let mut work_items: Vec<FetchTask> = Vec::new();
    let mut n_direct = 0usize;
    let mut n_grpc = 0usize;
    for batch in &request_batches {
        let (direct_batch, grpc_batch) = split_batch_by_direct_url(batch);
        if let Some(b) = direct_batch {
            n_direct += 1;
            work_items.push(FetchTask::Direct(b));
        }
        if let Some(b) = grpc_batch {
            n_grpc += 1;
            work_items.push(FetchTask::Grpc(b));
        }
    }

    if n_grpc == 0 {
        span.record("fetch_strategy", "direct");
    } else {
        span.record(
            "fetch_strategy",
            format!("hybrid(direct={n_direct},grpc={n_grpc})"),
        );
    }
    re_log::debug!("Fetch tasks: {n_direct} direct, {n_grpc} gRPC fallback");

    let http_client = reqwest::Client::new();

    let fetch_stream = futures::stream::iter(work_items.into_iter().enumerate())
        .map(|(task_idx, task)| {
            let http_client = http_client.clone();
            let client = client.clone();
            let pending_analytics = pending_analytics.clone();
            async move {
                let chunks = match task {
                    FetchTask::Direct(batch) => {
                        let bytes = batch_byte_size(&batch);
                        let chunks = fetch_batch_direct(&batch, &http_client).await?;
                        if let Some(analytics) = &pending_analytics {
                            analytics.record_direct_fetch(bytes);
                        }
                        chunks
                    }
                    FetchTask::Grpc(batch) => {
                        let bytes = batch_byte_size(&batch);
                        #[cfg(not(target_arch = "wasm32"))]
                        crate::chunk_fetcher::metrics::record_grpc_no_direct_urls(bytes);
                        let chunks =
                            fetch_batch_group_via_grpc(std::slice::from_ref(&batch), &client)
                                .await?;
                        if let Some(analytics) = &pending_analytics {
                            analytics.record_grpc_fetch(bytes);
                        }
                        chunks
                    }
                };
                Ok::<_, ApiError>((task_idx, chunks))
            }
            .instrument(tracing::info_span!("fetch_task", task_idx))
        })
        .buffer_unordered(IO_PIPELINE_BUFFER);

    tokio::pin!(fetch_stream);

    let mut next_to_emit: usize = 0;
    let mut reorder_buf: BTreeMap<usize, Vec<ChunksWithSegment>> = BTreeMap::new();

    while let Some(result) = fetch_stream.next().await {
        let (task_idx, chunks) = result?;
        reorder_buf.insert(task_idx, chunks);

        // Drain contiguous completed tasks in order
        while let Some(chunks) = reorder_buf.remove(&next_to_emit) {
            if !send_sorted_chunks(chunks, &global_segment_order, &output_channel).await {
                return Ok(());
            }
            next_to_emit += 1;
        }
    }

    // Fetch stats are already recorded per-task into pending_analytics.
    // The combined event will be sent when the last PendingQueryAnalytics clone is dropped.

    Ok(())
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

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        #[cfg(not(target_arch = "wasm32"))]
        let _trace_guard = attach_trace_context(self.trace_headers.as_ref());
        let _span = tracing::debug_span!("execute", partition).entered();

        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);

        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let chunk_infos = self
            .chunk_info
            .iter()
            .filter(|(segment_id, _)| {
                let hash_value = segment_id.hash_one(&random_state) as usize;
                hash_value % self.target_partitions == partition
            })
            // we end up with 1 batch per (rerun) segment. Order is important and must be preserved.
            // See SegmentStreamExec::try_new for details on ordering.
            .map(|(_, batches)| re_arrow_util::concat_polymorphic_batches(batches))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    None,
                    err,
                    "concatenating chunk-info batches per segment",
                )
                .into_df_error()
            })?;

        // if no chunks match this datafusion partition, return an empty stream
        if chunk_infos.is_empty() {
            let stream: DataframeSegmentStream<T> = DataframeSegmentStream { inner: None };
            return Ok(Box::pin(stream));
        }

        let client = self.client.clone();

        let (batches_tx, batches_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);
        let query_expression = self.query_expression.clone();
        let projected_schema = self.projected_schema.clone();
        let limit = self.limit;
        let cpu_join_handle = Some(
            self.worker_runtime.handle().spawn(
                chunk_store_cpu_worker_thread(
                    chunk_rx,
                    batches_tx,
                    query_expression,
                    projected_schema,
                    self.index_values.clone(),
                    limit,
                )
                .instrument(tracing::info_span!("cpu_worker")),
            ),
        );

        let stream = DataframeSegmentStreamInner {
            projected_schema: self.projected_schema.clone(),
            store_output_channel: batches_rx,
            client,
            chunk_infos,
            chunk_tx: Some(chunk_tx),
            io_join_handle: None,
            cpu_join_handle,
            cpu_runtime: Arc::clone(&self.worker_runtime),
            trace_headers: self.trace_headers.clone(),
            server_trace_id: self.server_trace_id,
            pending_analytics: self.pending_analytics.clone(),
        };
        let stream = DataframeSegmentStream {
            inner: Some(stream),
        };

        Ok(Box::pin(stream))
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
            limit: self.limit,
            trace_headers: self.trace_headers.clone(),
            server_trace_id: self.server_trace_id,
            pending_analytics: self.pending_analytics.clone(),
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{FixedSizeBinaryBuilder, UInt64Array};
    use arrow::datatypes::Field;

    use super::*;

    /// Extract segment ID from a chunk result (test helper)
    fn extract_segment_id_from_chunk((segment_id, _chunks): &SortedChunksWithSegment) -> &str {
        segment_id
    }

    /// Helper to create a test `RecordBatch` with chunk info for testing
    fn create_test_chunk_info(segment_id: &str, chunk_sizes: &[u64]) -> RecordBatch {
        let num_chunks = chunk_sizes.len();

        // Create segment ID column (all rows have same segment)
        let segment_ids = StringArray::from(vec![segment_id; num_chunks]);

        // Create chunk sizes column
        let sizes = UInt64Array::from(chunk_sizes.to_vec());

        // Create dummy chunk IDs
        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(num_chunks, 16);
        for i in 0..num_chunks {
            let mut id_bytes = [0u8; 16];
            id_bytes[0..4].copy_from_slice(&(i as u32).to_le_bytes());
            chunk_id_builder.append_value(id_bytes).unwrap();
        }
        let chunk_ids = chunk_id_builder.finish();

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                re_protos::cloud::v1alpha1::QueryDatasetResponse::field_chunk_segment_id()
                    .as_ref()
                    .clone(),
                Field::new(
                    re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH,
                    arrow::datatypes::DataType::UInt64,
                    false,
                ),
                re_protos::cloud::v1alpha1::QueryDatasetResponse::field_chunk_id()
                    .as_ref()
                    .clone(),
            ],
            HashMap::default(),
        ));

        RecordBatch::try_new_with_options(
            schema,
            vec![Arc::new(segment_ids), Arc::new(sizes), Arc::new(chunk_ids)],
            &RecordBatchOptions::new().with_row_count(Some(num_chunks)),
        )
        .unwrap()
    }

    #[test]
    fn test_create_request_batches_single_small_segment() {
        let chunk_info = create_test_chunk_info("seg1", &[100, 200, 300]); // 600 bytes total
        let target_size = 1000; // 1KB target

        let (batches, segment_order) =
            create_request_batches(vec![chunk_info], target_size).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 3);
        assert_eq!(segment_order, vec!["seg1"]);
    }

    #[test]
    fn test_create_request_batches_single_large_segment() {
        let chunk_info = create_test_chunk_info("seg1", &[300, 400, 500, 600]); // 1800 bytes total
        let target_size = 1000; // 1KB target

        let (batches, segment_order) =
            create_request_batches(vec![chunk_info], target_size).unwrap();

        // should be split into 3 as each batch should be under 1KB
        assert_eq!(batches.len(), 3);
        assert_eq!(segment_order, vec!["seg1"]);
    }

    #[test]
    fn test_create_request_batches_multiple_small_segments() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[100, 150]), // 250 bytes
            create_test_chunk_info("seg2", &[200, 250]), // 450 bytes
            create_test_chunk_info("seg3", &[300]),      // 300 bytes
            create_test_chunk_info("seg4", &[100]),      // 100 bytes
        ];
        let target_size = 800; // Should fit seg1+seg2 in first batch, seg3+seg4 in second

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].num_rows(), 4);
        assert_eq!(batches[1].num_rows(), 2);
        assert_eq!(segment_order, vec!["seg1", "seg2", "seg3", "seg4"]);
    }

    #[test]
    fn test_create_request_batches_mixed_small_and_large() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[100, 200]), // 300 bytes - small
            create_test_chunk_info("seg2", &[800, 900, 700]), // 2400 bytes - large, needs splitting
            create_test_chunk_info("seg3", &[150]),      // 150 bytes - small
        ];
        let target_size = 1000;

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        // Should have: [seg1], [seg2_part1], [seg2_part2], [seg2_part3], [seg3]
        assert_eq!(batches.len(), 5);
        assert_eq!(segment_order, vec!["seg1", "seg2", "seg3"]);
    }

    #[test]
    fn test_segment_order_within_batches_is_preserved() {
        let chunk_infos = vec![
            create_test_chunk_info("segA", &[100]), // First in input
            create_test_chunk_info("segB", &[200]), // Second in input
            create_test_chunk_info("segC", &[300]), // Third in input
        ];
        let target_size = 1000; // All segments fit in one batch

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 3);
        assert_eq!(segment_order, vec!["segA", "segB", "segC"]);

        // Verify that segments within the batch maintain input order
        let segment_id_column = batches[0]
            .column_by_name(
                re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID,
            )
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        let batch_segment_ids: Vec<String> = (0..segment_id_column.len())
            .map(|i| segment_id_column.value(i).to_owned())
            .collect();

        assert_eq!(batch_segment_ids, vec!["segA", "segB", "segC"]);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_simple_case() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // Simple case: one segment per response
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        let chunks: Vec<ChunksWithSegment> = vec![
            vec![(empty_chunk.clone(), Some("segC".to_owned()))],
            vec![(empty_chunk.clone(), Some("segA".to_owned()))],
            vec![(empty_chunk.clone(), Some("segB".to_owned()))],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // Verify chunks are sorted according to segment order
        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_multi_segment_response() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        let chunks: Vec<ChunksWithSegment> = vec![
            // Single response containing segments in wrong order: segC, segA, segB
            vec![
                (empty_chunk.clone(), Some("segC".to_owned())),
                (empty_chunk.clone(), Some("segC".to_owned())), // Multiple chunks for segC
                (empty_chunk.clone(), Some("segA".to_owned())),
                (empty_chunk.clone(), Some("segB".to_owned())),
                (empty_chunk.clone(), Some("segB".to_owned())), // Multiple chunks for segB
                (empty_chunk.clone(), Some("segA".to_owned())), // More chunks for segA
                (empty_chunk.clone(), Some("segB".to_owned())), // More chunks for segB
            ],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // After sorting, we should have segments in correct order: segA, segB, segC
        // And the function should have split the multi-segment response into separate responses
        assert_eq!(sorted_chunks.len(), 3);
        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify each segment has the correct number of chunks
        let seg_a_chunks = sorted_chunks[0].1.len();
        let seg_b_chunks = sorted_chunks[1].1.len();
        let seg_c_chunks = sorted_chunks[2].1.len();

        assert_eq!(seg_a_chunks, 2);
        assert_eq!(seg_b_chunks, 3);
        assert_eq!(seg_c_chunks, 2);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_mixed_responses() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // We have some single-segment responses, some multi-segment responses
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        let chunks: Vec<ChunksWithSegment> = vec![
            // Single segment response
            vec![(empty_chunk.clone(), Some("segC".to_owned()))],
            // Multi-segment response
            vec![
                (empty_chunk.clone(), Some("segB".to_owned())),
                (empty_chunk.clone(), Some("segA".to_owned())),
            ],
            // Another single segment response
            vec![(empty_chunk.clone(), Some("segB".to_owned()))],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // Should be sorted: segA, segB (grouped together), segC
        assert_eq!(sorted_chunks.len(), 3);

        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify segB has 2 chunks (they should be grouped together)
        let seg_b_chunks = sorted_chunks[1].1.len();
        assert_eq!(seg_b_chunks, 2);
    }
}
