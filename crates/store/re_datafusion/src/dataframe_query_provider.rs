use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

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
use futures_util::Stream;
use re_dataframe::external::re_chunk::Chunk;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{
    ChunkStoreHandle, Index, QueryCache, QueryEngine, QueryExpression, QueryHandle, StorageEngine,
};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::{FetchChunksRequest, ScanSegmentTableResponse};
use re_redap_client::{ApiResult, ConnectionClient};
use re_sorbet::{ColumnDescriptor, ColumnSelector};
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::Instrument as _;

use crate::dataframe_query_common::{
    align_record_batch_to_schema, group_chunk_infos_by_segment_id, prepend_string_column_schema,
};

/// This parameter sets the back pressure that either the streaming provider
/// can place on the CPU worker thread or the CPU worker thread can place on
/// the IO stream.
const CPU_THREAD_IO_CHANNEL_SIZE: usize = 32;

/// Target batch size in bytes for grouping segments together in requests.
/// This reduces the number of round-trips while keeping memory usage bounded.
const TARGET_BATCH_SIZE_BYTES: usize = 16 * 1024 * 1024; // 16 MB

/// Helper to attach parent trace context if available.
/// Returns a guard that must be kept alive for the duration of the traced scope.
/// We can use this to ensure all phases of table provider's execution pipeline are
/// parented by a single trace.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
fn attach_trace_context(
    trace_headers: &Option<crate::TraceHeaders>,
) -> Option<re_perf_telemetry::external::opentelemetry::ContextGuard> {
    let headers = trace_headers.as_ref()?;
    if !headers.traceparent.is_empty() {
        let parent_ctx =
            re_perf_telemetry::external::opentelemetry::global::get_text_map_propagator(|prop| {
                prop.extract(headers)
            });
        Some(parent_ctx.attach())
    } else {
        None
    }
}

#[derive(Debug)]
pub(crate) struct SegmentStreamExec {
    props: PlanProperties,
    chunk_info_batches: Arc<Vec<RecordBatch>>,

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
    client: ConnectionClient,

    /// passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    trace_headers: Option<crate::TraceHeaders>,
}

type ChunksWithSegment = Vec<(Chunk, Option<String>)>;

pub struct DataframeSegmentStreamInner {
    projected_schema: SchemaRef,
    client: ConnectionClient,
    chunk_infos: Vec<RecordBatch>,

    chunk_tx: Option<Sender<ApiResult<ChunksWithSegment>>>,
    store_output_channel: Receiver<RecordBatch>,
    io_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,

    /// We must keep a handle on the cpu runtime because the execution plan
    /// is dropped during streaming. We need this handle to continue to exist
    /// so that our worker does not shut down unexpectedly.
    cpu_runtime: Arc<CpuRuntime>,
    cpu_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,

    /// passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    trace_headers: Option<crate::TraceHeaders>,
}

/// This is a temporary fix to minimize the impact of leaking memory
/// per issue <https://github.com/rerun-io/dataplatform/issues/1494>
/// The work around is to check for when the stream has exhausted and
/// to set the `inner` to None, thereby clearing the memory since
/// we are not properly getting a `drop` call from the upstream
/// FFI interface. When the upstream issue resolves, change
/// `DataframeSegmentStreamInner` back into `DataframeSegmentStream`
/// and delete this wrapper struct.
pub struct DataframeSegmentStream {
    inner: Option<DataframeSegmentStreamInner>,
}

impl Stream for DataframeSegmentStream {
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this_outer = self.get_mut();
        let Some(this) = this_outer.inner.as_mut() else {
            return Poll::Ready(None);
        };

        #[cfg(not(target_arch = "wasm32"))]
        let _trace_guard = attach_trace_context(&this.trace_headers);
        let _span = tracing::info_span!("poll_next").entered();

        // If we have any errors on the worker thread, we want to ensure we pass them up
        // through the stream.
        if this
            .cpu_join_handle
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(false)
        {
            let Some(join_handle) = this.cpu_join_handle.take() else {
                return Poll::Ready(Some(exec_err!("CPU join handle is None")));
            };
            let cpu_join_result = this.cpu_runtime.handle().block_on(join_handle);

            match cpu_join_result {
                Err(err) => return Poll::Ready(Some(exec_err!("{err}"))),
                Ok(Err(err)) => return Poll::Ready(Some(Err(err))),
                Ok(Ok(())) => {}
            }
        }

        // If this is the first call, we are uninitialized so create the io worker
        if this.io_join_handle.is_none() {
            let io_handle = Handle::current();

            // In order to properly drop the tx so the channel closes, do not clone it.
            let Some(chunk_tx) = this.chunk_tx.take() else {
                return Poll::Ready(Some(exec_err!("No tx for chunks from CPU thread")));
            };

            let client = this.client.clone();
            let chunk_infos = this.chunk_infos.clone();
            let current_span = tracing::Span::current();

            let target_size = std::env::var("RERUN_DF_CHUNK_STREAM_BATCH_SIZE_MB")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or(TARGET_BATCH_SIZE_BYTES);

            let target_concurrency = std::env::var("RERUN_DF_CHUNK_STREAM_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(4);

            this.io_join_handle = Some(
                io_handle.spawn(
                    async move {
                        chunk_stream_io_loop(
                            client,
                            chunk_infos,
                            chunk_tx,
                            target_size,
                            target_concurrency,
                        )
                        .await
                    }
                    .instrument(current_span.clone()),
                ),
            );
        }

        let result = this
            .store_output_channel
            .poll_recv(cx)
            .map(|result| Ok(result).transpose());

        if matches!(&result, Poll::Ready(None)) {
            this_outer.inner = None;
        }

        result
    }
}

impl RecordBatchStream for DataframeSegmentStream {
    fn schema(&self) -> SchemaRef {
        self.inner
            .as_ref()
            .map(|inner| inner.projected_schema.clone())
            .unwrap_or_else(|| Schema::empty().into())
    }
}

impl SegmentStreamExec {
    #[tracing::instrument(level = "info", skip_all)]
    #[expect(clippy::too_many_arguments)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        num_partitions: usize,
        chunk_info_batches: Arc<Vec<RecordBatch>>,
        mut query_expression: QueryExpression,
        client: ConnectionClient,
        trace_headers: Option<crate::TraceHeaders>,
    ) -> datafusion::common::Result<Self> {
        let projected_schema = match projection {
            Some(p) => Arc::new(table_schema.project(p)?),
            None => Arc::clone(table_schema),
        };

        if let Some(projected_cols) = projection
            && !projected_cols.is_empty()
        {
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

        let chunk_info = group_chunk_infos_by_segment_id(&chunk_info_batches)?;

        let worker_runtime = Arc::new(CpuRuntime::try_new(num_partitions)?);

        Ok(Self {
            props,
            chunk_info_batches,
            chunk_info,
            query_expression,
            projected_schema,
            target_partitions: num_partitions,
            worker_runtime,
            client,
            trace_headers,
        })
    }
}

#[tracing::instrument(level = "trace", skip_all)]
async fn send_next_row(
    query_handle: &QueryHandle<StorageEngine>,
    segment_id: &str,
    target_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
) -> Result<Option<()>, DataFusionError> {
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
        return plan_err!("Unexpected number of columns returned from query");
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
    )?;

    let output_batch = align_record_batch_to_schema(&batch, target_schema)?;

    output_channel
        .send(output_batch)
        .await
        .map_err(|err| exec_datafusion_err!("{err}"))?;

    Ok(Some(()))
}

// TODO(#10781) - support for sending intermediate results/chunks
#[tracing::instrument(level = "trace", skip_all)]
async fn chunk_store_cpu_worker_thread(
    mut input_channel: Receiver<ApiResult<ChunksWithSegment>>,
    output_channel: Sender<RecordBatch>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
) -> Result<(), DataFusionError> {
    let mut current_stores: Option<(String, ChunkStoreHandle, QueryHandle<StorageEngine>)> = None;
    while let Some(chunks_and_segment_ids) = input_channel.recv().await {
        let chunks_and_segment_ids =
            chunks_and_segment_ids.map_err(|err| exec_datafusion_err!("{err}"))?;

        for (chunk, segment_id) in chunks_and_segment_ids {
            let segment_id = segment_id
                .ok_or_else(|| exec_datafusion_err!("Received chunk without a segment id"))?;

            if let Some((current_segment, _, query_handle)) = &current_stores {
                // When we change segments, flush the outputs
                if current_segment != &segment_id {
                    while send_next_row(
                        query_handle,
                        current_segment.as_str(),
                        &projected_schema,
                        &output_channel,
                    )
                    .await?
                    .is_some()
                    {}

                    current_stores = None;
                }
            }

            let current_stores = current_stores.get_or_insert({
                let store_id = StoreId::random(
                    StoreKind::Recording,
                    ApplicationId::from(segment_id.as_str()),
                );
                let store = ChunkStore::new_handle(store_id.clone(), Default::default());

                let query_engine =
                    QueryEngine::new(store.clone(), QueryCache::new_handle(store.clone()));
                let query_handle = query_engine.query(query_expression.clone());

                (segment_id.clone(), store, query_handle)
            });

            let (_, store, _) = current_stores;

            store
                .write()
                .insert_chunk(&Arc::new(chunk))
                .map_err(|err| exec_datafusion_err!("{err}"))?;
        }
    }

    // Flush out remaining of last segment
    if let Some((final_segment, _, query_handle)) = &mut current_stores.as_mut() {
        while send_next_row(
            query_handle,
            final_segment,
            &projected_schema,
            &output_channel,
        )
        .await?
        .is_some()
        {}
    }

    Ok(())
}

/// Extract segment ID from a `chunk_info` `RecordBatch`. Each `chunk_info` batch contains
/// chunks for a single segment, so we can just take the first row's `segment_id`.
fn extract_segment_id(chunk_info: &RecordBatch) -> Result<String, DataFusionError> {
    let segment_ids = chunk_info
        .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID)
        .ok_or_else(|| exec_datafusion_err!("Missing segment_id column"))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| exec_datafusion_err!("segment_id column is not a string array"))?;

    Ok(segment_ids.value(0).to_owned())
}

/// Estimate the total size of all chunks in a `chunk_info` `RecordBatch` by summing
/// the `chunk_byte_len` values for all rows.
fn estimate_segment_size(chunk_info: &RecordBatch) -> Result<u64, DataFusionError> {
    let chunk_sizes = chunk_info
        .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)
        .ok_or_else(|| exec_datafusion_err!("Missing chunk_byte_len column"))?
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| exec_datafusion_err!("chunk_byte_len column is not a uint64 array"))?;

    let total_size = (0..chunk_sizes.len()).map(|i| chunk_sizes.value(i)).sum();

    Ok(total_size)
}

type BatchingResult = (Vec<Vec<RecordBatch>>, Vec<String>);

/// Groups `chunk_infos` into batches targeting the specified size, with special handling
/// for segments larger than the target size (which get split).
///
/// Returns (batches, `segment_order`) where:
/// - batches: Vec of batches, each containing `chunk_infos` for one request
/// - `segment_order`: Original order of segments for preserving segment order
fn create_request_batches(
    chunk_infos: Vec<RecordBatch>,
    target_size: u64,
) -> Result<BatchingResult, DataFusionError> {
    let mut request_batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_batch_size = 0u64;
    let mut segment_order = Vec::new();

    for chunk_info in chunk_infos {
        let segment_id = extract_segment_id(&chunk_info)?;
        let segment_size = estimate_segment_size(&chunk_info)?;

        // Track original segment order
        if !segment_order.contains(&segment_id) {
            segment_order.push(segment_id.clone());
        }

        // Check if this segment would make the current batch too large
        if !current_batch.is_empty() && current_batch_size + segment_size > target_size {
            // Send current batch and start a new one
            request_batches.push(current_batch);
            current_batch = Vec::new();
            current_batch_size = 0;
        }

        // Special handling for segments larger than target size
        if segment_size > target_size {
            // If current batch is not empty, send it first
            if !current_batch.is_empty() {
                request_batches.push(current_batch);
                current_batch = Vec::new();
                current_batch_size = 0;
            }

            // Split the large segment into multiple requests
            let split_batches = split_large_segments(&segment_id, &chunk_info, target_size)?;
            for split_batch in split_batches {
                request_batches.push(vec![split_batch]);
            }
        } else {
            // Add segment to current batch
            current_batch.push(chunk_info);
            current_batch_size += segment_size;
        }
    }

    // Don't forget the last batch
    if !current_batch.is_empty() {
        request_batches.push(current_batch);
    }

    Ok((request_batches, segment_order))
}

/// Split an overly large segment into multiple smaller requests. Each request will contain
/// a subset of the chunks from the original segment, targeting approximately the batch size.
fn split_large_segments(
    segment_id: &str,
    chunk_info: &RecordBatch,
    target_size: u64,
) -> Result<Vec<RecordBatch>, DataFusionError> {
    let chunk_sizes = chunk_info
        .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)
        .ok_or_else(|| exec_datafusion_err!("Missing chunk_byte_len column"))?
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| exec_datafusion_err!("chunk_byte_len column is not a uint64 array"))?;

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
            let indices_array = arrow::array::UInt32Array::from(
                current_indices
                    .iter()
                    .map(|&i| i as u32)
                    .collect::<Vec<_>>(),
            );
            let batch = arrow::compute::take_record_batch(chunk_info, &indices_array)?;
            result_batches.push(batch);

            // Start new batch with current chunk
            current_indices = vec![row_idx];
            current_size = chunk_size;
        }
    }

    // Don't forget the last batch
    if !current_indices.is_empty() {
        let indices_array = arrow::array::UInt32Array::from(
            current_indices
                .iter()
                .map(|&i| i as u32)
                .collect::<Vec<_>>(),
        );
        let batch = arrow::compute::take_record_batch(chunk_info, &indices_array)?;
        result_batches.push(batch);
    }

    tracing::debug!(
        "Split large segment '{}' ({} bytes) into {} requests",
        segment_id,
        estimate_segment_size(chunk_info)?,
        result_batches.len()
    );

    Ok(result_batches)
}

/// Helper function to sort chunks by segment order.
/// This function handles the fact server can return multiple segments
/// in a single `ChunksWithSegment` response, so we need to group them by segment
/// and then sort according to the original segment order.
fn sort_chunks_by_segment_order(chunks: &mut Vec<ChunksWithSegment>, segment_order: &[String]) {
    use std::collections::HashMap;

    // Collect all individual chunks grouped by segment ID
    let mut segment_groups: HashMap<String, Vec<(Chunk, Option<String>)>> = HashMap::new();

    // Extract all chunks and group by segment
    for chunks_with_segment in std::mem::take(chunks) {
        for (chunk, segment_id_opt) in chunks_with_segment {
            let segment_id = segment_id_opt
                .clone()
                .unwrap_or_else(|| "unknown".to_owned());
            segment_groups
                .entry(segment_id)
                .or_default()
                .push((chunk, segment_id_opt));
        }
    }

    // Now rebuild chunks in the correct segment order
    for segment_id in segment_order {
        if let Some(segment_chunks) = segment_groups.remove(segment_id) {
            chunks.push(segment_chunks);
        }
    }
}

/// This is the function that will run on the IO (main) tokio runtime that will listen
/// to the gRPC channel for chunks coming in from the data platform. This loop is started
/// up by the execute fn of the physical plan, so we will start one per output DataFusion partition,
/// which is different from the Rerun `segment_id`. The sorting by time index will happen within
/// the cpu worker thread.
///
/// This optimized version batches multiple segments together into ~128MB requests to reduce
/// round-trips while preserving ordering through client-side buffering and reordering.
///
/// `chunk_infos` is a list of batches with chunk information where each batch has info for
/// a *single segment*. We also expect these to be previously sorted by segment id, otherwise
/// our suggestion to the query planner that inputs are sorted by segment id will be incorrect.
/// See `group_chunk_infos_by_segment_id` and `execute` for more details.
#[tracing::instrument(level = "trace", skip_all)]
async fn chunk_stream_io_loop(
    client: ConnectionClient,
    chunk_infos: Vec<RecordBatch>,
    output_channel: Sender<ApiResult<ChunksWithSegment>>,
    target_batch_size: usize,
    target_concurrency: usize,
) -> Result<(), DataFusionError> {
    // Build batches of requests to optimize network round-trips while maintaining ordering
    let target_size = target_batch_size as u64;
    let (request_batches, global_segment_order) = create_request_batches(chunk_infos, target_size)?;

    use futures::{StreamExt as _, TryStreamExt as _};

    // Process batches in chunks for memory efficiency while preserving perfect ordering
    for batch_group in request_batches.chunks(target_concurrency) {
        // Execute all batch requests in this group concurrently
        let group_results: Vec<Vec<ApiResult<ChunksWithSegment>>> =
            futures::stream::iter(batch_group.iter().cloned().map(|batch| {
                let mut client = client.clone();

                async move {
                    let chunk_infos_for_request: Vec<re_protos::common::v1alpha1::DataframePart> =
                        batch.into_iter().map(Into::into).collect();

                    let fetch_chunks_request = FetchChunksRequest {
                        chunk_infos: chunk_infos_for_request,
                    };

                    let fetch_chunks_response_stream = client
                        .inner()
                        .fetch_chunks(fetch_chunks_request)
                        .instrument(tracing::trace_span!("batched_fetch_chunks"))
                        .await
                        .map_err(|err| exec_datafusion_err!("{err}"))?
                        .into_inner();

                    // Collect all chunks from this single batch request
                    let chunk_stream =
                        re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(
                            fetch_chunks_response_stream,
                        );

                    let batch_chunks: Vec<ApiResult<ChunksWithSegment>> =
                        chunk_stream.collect().await;

                    Ok::<Vec<ApiResult<ChunksWithSegment>>, DataFusionError>(batch_chunks)
                }
            }))
            .buffer_unordered(target_concurrency)
            .try_collect()
            .await?;

        let mut all_chunks: Vec<ChunksWithSegment> = group_results
            .into_iter()
            .flatten()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| exec_datafusion_err!("Error fetching chunks: {err}"))?;

        // Sort chunks from this group using the global segment order
        sort_chunks_by_segment_order(&mut all_chunks, &global_segment_order);

        // Send all chunks from this group in correct order before processing next group
        for chunks_with_segment in all_chunks {
            if output_channel.send(Ok(chunks_with_segment)).await.is_err() {
                return Ok(());
            }
        }
    }

    Ok(())
}

impl ExecutionPlan for SegmentStreamExec {
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
        let _trace_guard = attach_trace_context(&self.trace_headers);
        let _span = tracing::info_span!("execute").entered();

        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);

        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let (_, chunk_infos): (Vec<_>, Vec<_>) = self
            .chunk_info
            .iter()
            .filter(|(segment_id, _)| {
                let hash_value = segment_id.hash_one(&random_state) as usize;
                hash_value % self.target_partitions == partition
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .unzip();
        // we end up with 1 batch per (rerun) segment. Order is important and must be preserved.
        // See SegmentStreamExec::try_new for details on ordering.
        let chunk_infos = chunk_infos
            .into_iter()
            .map(|batches| re_arrow_util::concat_polymorphic_batches(&batches))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        // if no chunks match this datafusion partition, return an empty stream
        if chunk_infos.is_empty() {
            let stream = DataframeSegmentStream { inner: None };
            return Ok(Box::pin(stream));
        }

        let client = self.client.clone();

        let (batches_tx, batches_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);
        let query_expression = self.query_expression.clone();
        let projected_schema = self.projected_schema.clone();
        let cpu_join_handle = Some(self.worker_runtime.handle().spawn(
            chunk_store_cpu_worker_thread(chunk_rx, batches_tx, query_expression, projected_schema),
        ));

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
            chunk_info_batches: self.chunk_info_batches.clone(),
            chunk_info: self.chunk_info.clone(),
            query_expression: self.query_expression.clone(),
            projected_schema: self.projected_schema.clone(),
            target_partitions,
            worker_runtime: Arc::new(CpuRuntime::try_new(target_partitions)?),
            client: self.client.clone(),
            trace_headers: self.trace_headers.clone(),
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

impl DisplayAs for SegmentStreamExec {
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
                log::error!("Error joining CPU runtime thread");
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
    fn extract_segment_id_from_chunk(chunk: &ChunksWithSegment) -> Option<String> {
        chunk.first()?.1.clone()
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

        assert_eq!(batches.len(), 1, "Should create 1 batch");
        assert_eq!(batches[0].len(), 1, "Batch should contain 1 segment");
        assert_eq!(segment_order, vec!["seg1"], "Should preserve segment order");
    }

    #[test]
    fn test_create_request_batches_single_large_segment() {
        let chunk_info = create_test_chunk_info("seg1", &[300, 400, 500, 600]); // 1800 bytes total
        let target_size = 1000; // 1KB target

        let (batches, segment_order) =
            create_request_batches(vec![chunk_info], target_size).unwrap();

        // Should split the large segment
        assert!(
            batches.len() > 1,
            "Should split large segment into multiple batches"
        );
        assert_eq!(segment_order, vec!["seg1"], "Should preserve segment order");
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

        assert_eq!(batches.len(), 2, "Should create 2 batches");
        assert_eq!(
            batches[0].len(),
            2,
            "First batch should have 2 segments (seg1+seg2=700 bytes)"
        );
        assert_eq!(
            batches[1].len(),
            2,
            "Second batch should have 2 segments (seg3+seg4=400 bytes)"
        );
        assert_eq!(
            segment_order,
            vec!["seg1", "seg2", "seg3", "seg4"],
            "Should preserve segment order"
        );
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

        // Should have: [seg1], [seg2_part1], [seg2_part2], [seg3]
        assert!(
            batches.len() >= 3,
            "Should create multiple batches due to large segment"
        );
        assert_eq!(
            segment_order,
            vec!["seg1", "seg2", "seg3"],
            "Should preserve segment order"
        );
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

        assert_eq!(batches.len(), 1, "Should create 1 batch with all segments");
        assert_eq!(batches[0].len(), 3, "Batch should contain all 3 segments");
        assert_eq!(
            segment_order,
            vec!["segA", "segB", "segC"],
            "Should preserve input order"
        );

        // Verify that segments within the batch maintain input order
        // Extract segment IDs from the first batch
        let batch_segment_ids: Result<Vec<_>, _> =
            batches[0].iter().map(extract_segment_id).collect();
        let batch_segment_ids = batch_segment_ids.unwrap();

        assert_eq!(
            batch_segment_ids,
            vec!["segA", "segB", "segC"],
            "Segments within batch should maintain original input order"
        );
    }

    #[test]
    fn test_sort_chunks_by_segment_order_simple_case() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // Simple case: one segment per response
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();

        let mut chunks: Vec<ChunksWithSegment> = vec![
            vec![(empty_chunk.clone(), Some("segC".to_owned()))],
            vec![(empty_chunk.clone(), Some("segA".to_owned()))],
            vec![(empty_chunk.clone(), Some("segB".to_owned()))],
        ];

        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        sort_chunks_by_segment_order(&mut chunks, &segment_order);

        // Verify chunks are sorted according to segment order
        let sorted_segments: Vec<String> = chunks
            .iter()
            .map(|chunk| extract_segment_id_from_chunk(chunk).unwrap_or_default())
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_multi_segment_response() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // This is the key test: server returns multiple segments in a single ChunksWithSegment response
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();

        let mut chunks: Vec<ChunksWithSegment> = vec![
            // Single response containing segments in wrong order: segC, segA, segB
            vec![
                (empty_chunk.clone(), Some("segC".to_owned())),
                (empty_chunk.clone(), Some("segC".to_owned())), // Multiple chunks for segC
                (empty_chunk.clone(), Some("segA".to_owned())),
                (empty_chunk.clone(), Some("segB".to_owned())),
                (empty_chunk.clone(), Some("segB".to_owned())), // Multiple chunks for segB
                (empty_chunk.clone(), Some("segA".to_owned())), // More chunks for segA
            ],
        ];

        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        sort_chunks_by_segment_order(&mut chunks, &segment_order);

        // After sorting, we should have segments in correct order: segA, segB, segC
        // And the function should have split the multi-segment response into separate responses
        assert_eq!(
            chunks.len(),
            3,
            "Should have 3 separate segment responses after splitting"
        );

        let sorted_segments: Vec<String> = chunks
            .iter()
            .map(|chunk| extract_segment_id_from_chunk(chunk).unwrap_or_default())
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify each segment has the correct number of chunks
        let seg_a_chunks = chunks[0].len();
        let seg_b_chunks = chunks[1].len();
        let seg_c_chunks = chunks[2].len();

        assert_eq!(seg_a_chunks, 2, "SegA should have 2 chunks");
        assert_eq!(seg_b_chunks, 2, "SegB should have 2 chunks");
        assert_eq!(seg_c_chunks, 2, "SegC should have 2 chunks");
    }

    #[test]
    fn test_sort_chunks_by_segment_order_mixed_responses() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // Mixed case: some single-segment responses, some multi-segment responses
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();

        let mut chunks: Vec<ChunksWithSegment> = vec![
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

        let segment_order = vec!["segA".to_owned(), "segB".to_owned(), "segC".to_owned()];

        sort_chunks_by_segment_order(&mut chunks, &segment_order);

        // Should be sorted: segA, segB (grouped together), segC
        assert_eq!(
            chunks.len(),
            3,
            "Should have 3 separate segment responses after grouping"
        );

        let sorted_segments: Vec<String> = chunks
            .iter()
            .map(|chunk| extract_segment_id_from_chunk(chunk).unwrap_or_default())
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify segB has 2 chunks (they should be grouped together)
        let seg_b_chunks = chunks[1].len();
        assert_eq!(
            seg_b_chunks, 2,
            "segB should have 2 chunks grouped together"
        );
    }
}
