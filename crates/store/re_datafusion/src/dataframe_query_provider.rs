use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrow::array::{Array, RecordBatch, RecordBatchOptions, StringArray};
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
use futures_util::{Stream, StreamExt as _, TryStreamExt as _};
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
        let _span = tracing::debug_span!("poll_next").entered();

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

            this.io_join_handle = Some(
                io_handle.spawn(
                    async move { chunk_stream_io_loop(client, chunk_infos, chunk_tx).await }
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

/// This is the function that will run on the IO (main) tokio runtime that will listen
/// to the gRPC channel for chunks coming in from the data platform. This loop is started
/// up by the execute fn of the physical plan, so we will start one per output DataFusion partition,
/// which is different from the Rerun `segment_id`. The sorting by time index will happen within
/// the cpu worker thread.
/// `chunk_infos` is a list of batches with chunk information where each batch has info for
/// a *single segment*. We also expect these to be previously sorted by segment id, otherwise
/// our suggestion to the query planner that inputs are sorted by segment id will be incorrect.
/// See `group_chunk_infos_by_segment_id` and `execute` for more details.
#[tracing::instrument(level = "trace", skip_all)]
async fn chunk_stream_io_loop(
    client: ConnectionClient,
    chunk_infos: Vec<RecordBatch>,
    output_channel: Sender<ApiResult<ChunksWithSegment>>,
) -> Result<(), DataFusionError> {
    // Pipeline configuration
    const CONCURRENT_REQUESTS: usize = 16;
    const BUFFER_SIZE_MB: usize = 512;
    const BUFFER_SIZE_BYTES: usize = BUFFER_SIZE_MB * 1024 * 1024;

    // Create intermediate channel for ordered buffering
    let (intermediate_tx, mut intermediate_rx) =
        tokio::sync::mpsc::channel::<(usize, ApiResult<ChunksWithSegment>, u64)>(1024);

    // We need to pair original RecordBatch with converted ChunkInfo for byte length extraction
    use re_protos::common::v1alpha1::DataframePart;
    let chunk_info_pairs: Vec<_> = chunk_infos
        .into_iter()
        .enumerate()
        .map(|(index, batch)| {
            let chunk_info: DataframePart = batch.clone().into();
            (index, batch, chunk_info)
        })
        .collect();

    // Spawn concurrent fetchers
    let fetcher_handle = tokio::spawn(async move {
        futures_util::stream::iter(chunk_info_pairs)
            .map(|(index, original_batch, chunk_info)| {
                let mut client = client.clone();
                let intermediate_tx = intermediate_tx.clone();
                async move {
                    let fetch_chunks_request = FetchChunksRequest {
                        chunk_infos: vec![chunk_info],
                    };

                    let fetch_chunks_response_stream = client
                        .inner()
                        .fetch_chunks(fetch_chunks_request)
                        .instrument(tracing::trace_span!("chunk_stream_io_loop"))
                        .await
                        .map_err(|err| exec_datafusion_err!("{err}"))?
                        .into_inner();

                    let mut chunk_stream =
                        re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(
                            fetch_chunks_response_stream,
                        );

                    while let Some(chunk_and_segment_id) = chunk_stream.next().await {
                        // Extract byte length from original RecordBatch
                        let byte_len = extract_chunk_byte_len(&original_batch)?;

                        if intermediate_tx
                            .send((index, chunk_and_segment_id, byte_len))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Ok::<(), DataFusionError>(())
                }
            })
            .buffered(CONCURRENT_REQUESTS)
            .try_collect::<Vec<_>>()
            .await
    });

    // Spawn ordered buffer manager
    let buffer_handle = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let mut total_bytes = 0u64;

        while let Some((index, chunk_result, byte_len)) = intermediate_rx.recv().await {
            buffer.push((index, chunk_result, byte_len));
            total_bytes += byte_len;

            // Check if we should flush (either buffer size reached or no more data expected)
            if total_bytes >= BUFFER_SIZE_BYTES as u64 || intermediate_rx.is_closed() {
                // Sort buffer by original index to preserve input ordering
                buffer.sort_by_key(|(index, _, _)| *index);

                // Flush ordered chunks to output
                for (_, chunk_result, _) in buffer.drain(..) {
                    if output_channel.send(chunk_result).await.is_err() {
                        return Ok(());
                    }
                }

                total_bytes = 0;
            }
        }

        // Flush any remaining chunks
        if !buffer.is_empty() {
            buffer.sort_by_key(|(index, _, _)| *index);
            for (_, chunk_result, _) in buffer.drain(..) {
                if output_channel.send(chunk_result).await.is_err() {
                    break;
                }
            }
        }

        Ok::<(), DataFusionError>(())
    });

    // Wait for both tasks to complete
    let (fetcher_result, buffer_result) = tokio::try_join!(fetcher_handle, buffer_handle)
        .map_err(|err| exec_datafusion_err!("Task join error: {err}"))?;

    fetcher_result?;
    buffer_result?;

    Ok(())
}

fn extract_chunk_byte_len(chunk_info_batch: &RecordBatch) -> Result<u64, DataFusionError> {
    use arrow::array::AsArray as _;
    use re_protos::cloud::v1alpha1::FetchChunksRequest;

    // Find the chunk_byte_len column in the batch
    let schema = chunk_info_batch.schema();
    let chunk_byte_len_col_idx = schema
        .column_with_name(FetchChunksRequest::FIELD_CHUNK_BYTE_LEN)
        .ok_or_else(|| {
            exec_datafusion_err!(
                "Missing {} column in chunk info",
                FetchChunksRequest::FIELD_CHUNK_BYTE_LEN
            )
        })?
        .0;

    let chunk_byte_len_array = chunk_info_batch.column(chunk_byte_len_col_idx);

    // Assuming it's a UInt64 array with a single value (since we're processing one chunk at a time)
    let uint64_array = chunk_byte_len_array.as_primitive::<arrow::datatypes::UInt64Type>();

    if uint64_array.len() != 1 {
        return Err(exec_datafusion_err!(
            "Expected exactly one chunk_byte_len value, got {}",
            uint64_array.len()
        ));
    }

    Ok(uint64_array.value(0))
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
