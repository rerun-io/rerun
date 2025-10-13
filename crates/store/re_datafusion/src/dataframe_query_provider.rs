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
use datafusion::execution::{RecordBatchStream, TaskContext};
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{
    EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr, PhysicalSortExpr,
};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use datafusion::{error::DataFusionError, execution::SendableRecordBatchStream};
use futures_util::{Stream, StreamExt as _};
use re_log_encoding::codec::wire::encoder::Encode as _;
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::Instrument as _;

use re_dataframe::external::re_chunk::Chunk;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{
    ChunkStoreHandle, Index, QueryCache, QueryEngine, QueryExpression, QueryHandle, StorageEngine,
};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::{FetchChunksRequest, ScanPartitionTableResponse};
use re_redap_client::ConnectionClient;
use re_sorbet::{ColumnDescriptor, ColumnSelector};

use crate::dataframe_query_common::{
    align_record_batch_to_schema, group_chunk_infos_by_partition_id, prepend_string_column_schema,
};

/// This parameter sets the back pressure that either the streaming provider
/// can place on the CPU worker thread or the CPU worker thread can place on
/// the IO stream.
const CPU_THREAD_IO_CHANNEL_SIZE: usize = 32;

#[derive(Debug)]
pub(crate) struct PartitionStreamExec {
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
}

type ChunksWithPartition = Vec<(Chunk, Option<String>)>;

pub struct DataframePartitionStreamInner {
    projected_schema: SchemaRef,
    client: ConnectionClient,
    chunk_infos: Vec<RecordBatch>,

    chunk_tx: Option<Sender<Result<ChunksWithPartition, re_redap_client::StreamError>>>,
    store_output_channel: Receiver<RecordBatch>,
    io_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,

    /// We must keep a handle on the cpu runtime because the execution plan
    /// is dropped during streaming. We need this handle to continue to exist
    /// so that our worker does not shut down unexpectedly.
    cpu_runtime: Arc<CpuRuntime>,
    cpu_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,
}

/// This is a temporary fix to minimize the impact of leaking memory
/// per issue <https://github.com/rerun-io/dataplatform/issues/1494>
/// The work around is to check for when the stream has exhausted and
/// to set the `inner` to None, thereby clearing the memory since
/// we are not properly getting a `drop` call from the upstream
/// FFI interface. When the upstream issue resolves, change
/// `DataframePartitionStreamInner` back into `DataframePartitionStream`
/// and delete this wrapper struct.
pub struct DataframePartitionStream {
    inner: Option<DataframePartitionStreamInner>,
}

impl Stream for DataframePartitionStream {
    type Item = Result<RecordBatch, DataFusionError>;

    #[tracing::instrument(level = "info", skip_all)]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this_outer = self.get_mut();
        let Some(this) = this_outer.inner.as_mut() else {
            return Poll::Ready(None);
        };

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

            this.io_join_handle = Some(io_handle.spawn(chunk_stream_io_loop(
                this.client.clone(),
                this.chunk_infos.clone(),
                chunk_tx,
            )));
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

impl RecordBatchStream for DataframePartitionStream {
    fn schema(&self) -> SchemaRef {
        self.inner
            .as_ref()
            .map(|inner| inner.projected_schema.clone())
            .unwrap_or(Schema::empty().into())
    }
}

impl PartitionStreamExec {
    #[tracing::instrument(level = "info", skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        num_partitions: usize,
        chunk_info_batches: Arc<Vec<RecordBatch>>,
        mut query_expression: QueryExpression,
        client: ConnectionClient,
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
        // partition ID and then time index. If the output does not have rerun
        // partition ID included, we cannot specify any output ordering.

        let orderings = if projected_schema
            .fields()
            .iter()
            .any(|f| f.name().as_str() == ScanPartitionTableResponse::FIELD_PARTITION_ID)
        {
            let partition_col = Arc::new(Column::new(
                ScanPartitionTableResponse::FIELD_PARTITION_ID,
                0,
            )) as Arc<dyn PhysicalExpr>;
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
                    ScanPartitionTableResponse::FIELD_PARTITION_ID,
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

        let chunk_info = group_chunk_infos_by_partition_id(&chunk_info_batches)?;

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
        })
    }
}

#[tracing::instrument(level = "trace", skip_all)]
async fn send_next_row(
    query_handle: &QueryHandle<StorageEngine>,
    partition_id: &str,
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
    let pid_array =
        Arc::new(StringArray::from(vec![partition_id.to_owned(); num_rows])) as Arc<dyn Array>;

    next_row.insert(0, pid_array);

    let batch_schema = Arc::new(prepend_string_column_schema(
        &query_schema,
        ScanPartitionTableResponse::FIELD_PARTITION_ID,
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
    mut input_channel: Receiver<Result<ChunksWithPartition, re_redap_client::StreamError>>,
    output_channel: Sender<RecordBatch>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
) -> Result<(), DataFusionError> {
    let mut current_stores: Option<(String, ChunkStoreHandle, QueryHandle<StorageEngine>)> = None;
    while let Some(chunks_and_partition_ids) = input_channel.recv().await {
        let chunks_and_partition_ids =
            chunks_and_partition_ids.map_err(|err| exec_datafusion_err!("{err}"))?;

        for (chunk, partition_id) in chunks_and_partition_ids {
            let partition_id = partition_id
                .ok_or_else(|| exec_datafusion_err!("Received chunk without a partition id"))?;

            if let Some((current_partition, _, query_handle)) = &current_stores {
                // When we change partitions, flush the outputs
                if current_partition != &partition_id {
                    while send_next_row(
                        query_handle,
                        current_partition.as_str(),
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
                    ApplicationId::from(partition_id.as_str()),
                );
                let store = ChunkStore::new_handle(store_id.clone(), Default::default());

                let query_engine =
                    QueryEngine::new(store.clone(), QueryCache::new_handle(store.clone()));
                let query_handle = query_engine.query(query_expression.clone());

                (partition_id.clone(), store, query_handle)
            });

            let (_, store, _) = current_stores;

            store
                .write()
                .insert_chunk(&Arc::new(chunk))
                .map_err(|err| exec_datafusion_err!("{err}"))?;
        }
    }

    // Flush out remaining of last partition
    if let Some((final_partition, _, query_handle)) = &mut current_stores.as_mut() {
        while send_next_row(
            query_handle,
            final_partition,
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
/// up by the execute fn of the physical plan, so we will start one per output partition,
/// which is different from the `partition_id`. The sorting by time index will happen within
/// the cpu worker thread.
/// `chunk_infos` is a list of batches with chunk information where each batch has info for
/// a *single partition*. We also expect these to be previously sorted by partition id, otherwise
/// our suggestion to the query planner that inputs are sorted by partition id will be incorrect.
/// See `group_chunk_infos_by_partition_id` and `execute` for more details.
#[tracing::instrument(level = "trace", skip_all)]
async fn chunk_stream_io_loop(
    mut client: ConnectionClient,
    chunk_infos: Vec<RecordBatch>,
    output_channel: Sender<Result<ChunksWithPartition, re_redap_client::StreamError>>,
) -> Result<(), DataFusionError> {
    let chunk_infos = chunk_infos
        .into_iter()
        .map(|batch| batch.encode())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| exec_datafusion_err!("{err}"))?;

    // TODO(zehiko) same as previously with get_chunks, we keep sending 1 request per partition.
    // As these batches are sorted per partition (see docs above), this ensures that ordering by
    // partition id is preserved regardless of how server might order responses (in the case of having
    // batches with different partitions in the same request). However, quick testing shows that this
    // is at least 2x slower than sending all partitions in one request. Consider providing ordering
    // guarantees server side in the future.
    for chunk_info in chunk_infos {
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

        // Then we need to fully decode these chunks, i.e. both the transport layer (Protobuf)
        // and the app layer (Arrow).
        let mut chunk_stream = re_redap_client::fetch_chunks_response_to_chunk_and_partition_id(
            fetch_chunks_response_stream,
        );

        while let Some(chunk_and_partition_id) = chunk_stream.next().await {
            if output_channel.send(chunk_and_partition_id).await.is_err() {
                break;
            }
        }
    }

    Ok(())
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
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        if children.is_empty() {
            Ok(self)
        } else {
            plan_err!("PartitionStreamExec does not support children")
        }
    }

    #[tracing::instrument(level = "info", skip_all)]
    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);

        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let (_, chunk_infos): (Vec<_>, Vec<_>) = self
            .chunk_info
            .iter()
            .filter(|(partition_id, _)| {
                let hash_value = partition_id.hash_one(&random_state) as usize;
                hash_value % self.target_partitions == partition
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .unzip();
        // we end up with 1 batch per (rerun) partition. Order is important and must be preserved.
        // See PartitionStreamExec::try_new for details on ordering.
        let chunk_infos = chunk_infos
            .into_iter()
            .map(|batches| re_arrow_util::concat_polymorphic_batches(&batches))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        // if no chunks match this datafusion partition, return an empty stream
        if chunk_infos.is_empty() {
            let stream = DataframePartitionStream { inner: None };
            return Ok(Box::pin(stream));
        }

        let client = self.client.clone();

        let (batches_tx, batches_rx) = tokio::sync::mpsc::channel(CPU_THREAD_IO_CHANNEL_SIZE);
        let query_expression = self.query_expression.clone();
        let projected_schema = self.projected_schema.clone();
        let cpu_join_handle = Some(self.worker_runtime.handle().spawn(
            chunk_store_cpu_worker_thread(chunk_rx, batches_tx, query_expression, projected_schema),
        ));

        let stream = DataframePartitionStreamInner {
            projected_schema: self.projected_schema.clone(),
            store_output_channel: batches_rx,
            client,
            chunk_infos,
            chunk_tx: Some(chunk_tx),
            io_join_handle: None,
            cpu_join_handle,
            cpu_runtime: Arc::clone(&self.worker_runtime),
        };
        let stream = DataframePartitionStream {
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

impl DisplayAs for PartitionStreamExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PartitionStreamExec: num_partitions={:?}",
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
