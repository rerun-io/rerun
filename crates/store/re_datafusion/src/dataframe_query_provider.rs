use arrow::array::{
    Array, ArrayRef, Int64Array, RecordBatch, StringArray, TimestampMicrosecondArray,
    TimestampMillisecondArray, TimestampNanosecondArray, TimestampSecondArray, UInt64Array,
    new_null_array,
};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Int64Type, Schema, SchemaRef, TimeUnit};
use async_trait::async_trait;
use datafusion::catalog::Session;
use datafusion::common::hash_utils::HashValue as _;
use datafusion::common::{downcast_value, exec_datafusion_err, exec_err, plan_err};
use datafusion::config::ConfigOptions;
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
use futures_util::{Stream, StreamExt as _};
use re_dataframe::external::re_chunk::Chunk;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{
    ChunkStoreHandle, Index, QueryCache, QueryEngine, QueryExpression, QueryHandle, StorageEngine,
};
use re_grpc_client::{ConnectionClient, ConnectionRegistryHandle};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::external::re_types_core::{ChunkId, Loggable as _};
use re_log_types::{ApplicationId, EntryId, StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::common::v1alpha1::PartitionId;
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::frontend::v1alpha1::{
    GetChunksRequest, GetDatasetSchemaRequest, QueryDatasetRequest,
};
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_protos::manifest_registry::v1alpha1::ext::Query;
use re_sorbet::{BatchType, ChunkColumnDescriptors, ColumnKind};
use re_tuid::Tuid;
use re_uri::Origin;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::runtime::Handle;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::Instrument as _;

/// Sets the size for output record batches in rows. The last batch will likely be smaller.
/// The default for Data Fusion is 8192, which leads to a 256Kb record batch on average for
/// rows with 32b of data. We are setting this lower as a reasonable first guess to avoid
/// the pitfall of executing a single row at a time, but we will likely want to consider
/// at some point moving to a dynamic sizing.
const DEFAULT_BATCH_SIZE: usize = 2048;
const DEFAULT_OUTPUT_PARTITIONS: usize = 14;

#[derive(Debug)]
pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    sort_index: Option<Index>,
    chunk_info_batches: Arc<Vec<RecordBatch>>,
    client: ConnectionClient,
    chunk_request: GetChunksRequest,
}

#[derive(Debug)]
struct PartitionStreamExec {
    props: PlanProperties,
    chunk_info_batches: Arc<Vec<RecordBatch>>,
    /// Describes the chunks per partition, derived from `chunk_info_batches`.
    /// We keep both around so that we only have to process once, but we may
    /// reuse multiple times in theory. We may also need to recompute if the
    /// user asks for a different target partition. These are generally not
    /// too large.
    chunk_info: Arc<BTreeMap<String, Vec<ChunkInfo>>>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
    target_partitions: usize,
    worker_runtime: Arc<CpuRuntime>,
    client: ConnectionClient,
    chunk_request: GetChunksRequest,
}

pub struct DataframePartitionStream {
    projected_schema: SchemaRef,
    client: ConnectionClient,
    chunk_request: GetChunksRequest,
    rerun_partition_ids: Vec<String>,

    chunk_tx: Option<Sender<Vec<(Chunk, Option<String>)>>>,
    store_output_channel: Receiver<RecordBatch>,
    io_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,

    /// We must keep a handle on the cpu runtime because the execution plan
    /// is dropped during streaming. We need this handle to continue to exist
    /// so that our worker does not shut down unexpectedly.
    cpu_runtime: Arc<CpuRuntime>,
    cpu_join_handle: Option<JoinHandle<Result<(), DataFusionError>>>,
}

/// Compute the output schema for a query on a dataset. When we call `get_dataset_schema`
/// on the data platform, we will get the schema for all entities and all components. This
/// method is used to down select from that full schema based on `query_expression`.
fn compute_schema_for_query(
    dataset_schema: &Schema,
    query_expression: &QueryExpression,
) -> Result<SchemaRef, DataFusionError> {
    // Schema returned from `get_dataset_schema` does not match the required ChunkColumnDescriptors ordering
    // which is row id, then time, then data. We don't need perfect ordering other than that.
    let mut fields = dataset_schema
        .fields()
        .iter()
        .map(Arc::clone)
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| {
        let Ok(a) = ColumnKind::try_from(a.as_ref()) else {
            return Ordering::Equal;
        };
        let Ok(b) = ColumnKind::try_from(b.as_ref()) else {
            return Ordering::Equal;
        };

        match (a, b) {
            (ColumnKind::RowId, _) => Ordering::Less,
            (_, ColumnKind::RowId) => Ordering::Greater,
            (ColumnKind::Index, _) => Ordering::Less,
            (_, ColumnKind::Index) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    let fields: arrow::datatypes::Fields = fields.into();

    let column_descriptors = ChunkColumnDescriptors::try_from_arrow_fields(None, &fields)
        .map_err(|err| exec_datafusion_err!("col desc {err}"))?;

    // Create the actual filter to apply to the column descriptors
    let filter = ChunkStore::create_component_filter_from_query(query_expression);

    // When we call GetChunks we will not return row_id, so we only select indices and
    // components from the column descriptors.
    let filtered_fields = column_descriptors
        .filter_components(filter)
        .indices_and_components()
        .into_iter()
        .map(|cd| cd.to_arrow_field(BatchType::Dataframe))
        .collect::<Vec<_>>();

    Ok(Arc::new(Schema::new_with_metadata(
        filtered_fields,
        dataset_schema.metadata().clone(),
    )))
}

impl DataframeQueryTableProvider {
    /// Create a table provider for a gRPC query. This function is async
    /// because we need to make gRPC calls to determine the schema at the
    /// creation of the table provider.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn new(
        // query_engines: BTreeMap<String, QueryEngine<StorageEngine>>,
        origin: Origin,
        connection: ConnectionRegistryHandle,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
    ) -> Result<Self, DataFusionError> {
        use futures::StreamExt as _;

        let mut client = connection
            .client(origin)
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let schema = client
            .inner()
            .get_dataset_schema(GetDatasetSchemaRequest {
                dataset_id: Some(dataset_id.into()),
            })
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner()
            .schema()
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let schema = compute_schema_for_query(&schema, query_expression)?;

        let select_all_entity_paths = false;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let fuzzy_descriptors: Vec<String> = query_expression
            .view_contents
            .as_ref()
            .map_or(BTreeSet::new(), |contents| {
                contents
                    .values()
                    .filter_map(|opt_set| opt_set.as_ref())
                    .flat_map(|set| set.iter().copied())
                    .collect::<BTreeSet<_>>()
            })
            .into_iter()
            .map(|ident| ident.to_string())
            .collect();

        let query = Query::from(query_expression);

        let mut fields_of_interest = [
            "chunk_partition_id",
            "chunk_entity_path",
            "chunk_id",
            "chunk_is_static",
            "chunk_byte_len",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

        if let Some(index) = query_expression.filtered_index {
            fields_of_interest.push(format!("{}:start", index.as_str()));
            fields_of_interest.push(format!("{}:end", index.as_str()));
        }

        let chunk_request = GetChunksRequest {
            dataset_id: Some(dataset_id.into()),
            partition_ids: vec![],
            chunk_ids: vec![],
            entity_paths: entity_paths.iter().map(|p| (*p).clone().into()).collect(),
            select_all_entity_paths,
            fuzzy_descriptors: fuzzy_descriptors.clone(),
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query.clone().into()),
        };

        let dataset_query = QueryDatasetRequest {
            dataset_id: Some(dataset_id.into()),
            partition_ids: vec![],
            chunk_ids: vec![],
            entity_paths: entity_paths
                .into_iter()
                .map(|p| (*p).clone().into())
                .collect(),
            select_all_entity_paths,
            fuzzy_descriptors,
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query.into()),
            scan_parameters: Some(
                ScanParameters {
                    columns: fields_of_interest,
                    ..Default::default()
                }
                .into(),
            ),
        };

        let response_stream = client
            .inner()
            .query_dataset(dataset_query)
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner();

        let chunk_info_batches = response_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_iter()
            .filter_map(|response| response.data)
            .map(|dataframe_part| {
                dataframe_part
                    .decode()
                    .map_err(|err| exec_datafusion_err!("{err}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into();

        let schema = Arc::new(prepend_string_column_schema(
            &schema,
            DATASET_MANIFEST_ID_FIELD_NAME,
        ));

        Ok(Self {
            schema,
            query_expression: query_expression.to_owned(),
            sort_index: query_expression.filtered_index,
            chunk_info_batches,
            client,
            chunk_request,
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
            Arc::clone(&self.chunk_info_batches),
            self.query_expression.clone(),
            self.client.clone(),
            self.chunk_request.clone(),
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
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // If we have any errors on the worker thread, we want to ensure we pass them up
        // through the stream.
        if this
            .cpu_join_handle
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(false)
        {
            let join_handle = this.cpu_join_handle.take().unwrap();
            let cpu_join_result = this.cpu_runtime.handle().block_on(join_handle);

            // let cpu_join_result = cpu_join_result.map_err(|err| exec_datafusion_err!("{err}"))
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
                this.chunk_request.clone(),
                this.rerun_partition_ids.clone(),
                chunk_tx,
            )));
        }

        this.store_output_channel
            .poll_recv(cx)
            .map(|result| Ok(result).transpose())
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChunkInfo {
    pub start_time: i64,
    pub end_time: i64,
    pub chunk_id: ChunkId,
    pub byte_len: u64,
}

impl Ord for ChunkInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        let start_time_cmp = self.start_time.cmp(&other.start_time);
        let Ordering::Equal = start_time_cmp else {
            return start_time_cmp;
        };
        let end_time_cmp = self.end_time.cmp(&other.end_time);
        let Ordering::Equal = end_time_cmp else {
            return end_time_cmp;
        };
        let chunk_id_cmp = self.chunk_id.cmp(&other.chunk_id);
        let Ordering::Equal = chunk_id_cmp else {
            return chunk_id_cmp;
        };

        // We should never get here
        self.byte_len.cmp(&other.byte_len)
    }
}
impl PartialOrd for ChunkInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn time_array_ref_to_i64(time_array: &ArrayRef) -> Result<Int64Array, DataFusionError> {
    Ok(match time_array.data_type() {
        DataType::Int64 => downcast_value!(time_array, Int64Array).reinterpret_cast::<Int64Type>(),
        DataType::Timestamp(TimeUnit::Second, _) => {
            let nano_array = downcast_value!(time_array, TimestampSecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampMillisecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampMicrosecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampNanosecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        _ => {
            return Err(exec_datafusion_err!(
                "Unexpected type for time column {}",
                time_array.data_type()
            ));
        }
    })
}

/// We need to create `num_partitions` of partition stream outputs, each of
/// which will be fed from multiple `rerun_partition_id` sources. The partitioning
/// output is a hash of the `rerun_partition_id`. We will reuse some of the
/// underlying execution code from DataFusion's `RepartitionExec` to compute
/// these partition IDs, just to be certain they match partitioning generated
/// from sources other than Rerun gRPC services. This will return a vector of
/// vector of tuple. The outer vector is of length `num_partitions`. The inner
/// vector contains the combination of `rerun_partition_id`, chunk ID, and
/// chunk byte length.
fn compute_partition_stream_chunk_info(
    chunk_info_batches: &Arc<Vec<RecordBatch>>,
) -> Result<Arc<BTreeMap<String, Vec<ChunkInfo>>>, DataFusionError> {
    let mut results = BTreeMap::new();

    for batch in chunk_info_batches.as_ref() {
        let schema = batch.schema();
        let end_time_col = schema
            .fields()
            .iter()
            .map(|f| f.name())
            .find(|name| name.ends_with(":end"))
            .ok_or(exec_datafusion_err!("Unable to identify time index"))?;
        let start_time_col = schema
            .fields()
            .iter()
            .map(|f| f.name())
            .find(|name| name.ends_with(":start"))
            .ok_or(exec_datafusion_err!("Unable to identify time index"))?;

        let partition_id_arr = batch
            .column_by_name("chunk_partition_id")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_partition_id as expected"
            ))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!("Unexpected type for chunk_id"))?;

        let chunk_id_arr = batch
            .column_by_name("chunk_id")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_id as expected"
            ))
            .and_then(|arr| Tuid::from_arrow(arr).map_err(|err| exec_datafusion_err!("{err}")))?;

        let chunk_byte_len_arr = batch
            .column_by_name("chunk_byte_len")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_byte_len as expected"
            ))?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Unexpected type for chunk_id"))?;

        let end_time_arr = batch
            .column_by_name(end_time_col)
            .ok_or(exec_datafusion_err!(
                "Unable to return end time column as expected"
            ))?;
        let end_time_arr = time_array_ref_to_i64(end_time_arr)?;
        let start_time_arr = batch
            .column_by_name(start_time_col)
            .ok_or(exec_datafusion_err!(
                "Unable to return start time column as expected"
            ))?;
        let start_time_arr = time_array_ref_to_i64(start_time_arr)?;

        let num_rows = partition_id_arr.len();
        for idx in 0..num_rows {
            let partition_id = partition_id_arr.value(idx).to_owned();
            // let hash_idx = partition_id.hash_one(&random_state) as usize;
            let chunk_id = ChunkId::from_tuid(chunk_id_arr[idx]);
            let byte_len = chunk_byte_len_arr.value(idx);
            let start_time = start_time_arr.value(idx);
            let end_time = end_time_arr.value(idx);

            let chunk_info = ChunkInfo {
                start_time,
                end_time,
                chunk_id,
                byte_len,
            };

            let chunks_vec = results.entry(partition_id).or_insert(vec![]);
            chunks_vec.push(chunk_info);
        }
    }

    Ok(Arc::new(results))
}

impl PartitionStreamExec {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn try_new(
        table_schema: &SchemaRef,
        sort_index: Option<Index>,
        projection: Option<&Vec<usize>>,
        chunk_info_batches: Arc<Vec<RecordBatch>>,
        query_expression: QueryExpression,
        client: ConnectionClient,
        chunk_request: GetChunksRequest,
    ) -> datafusion::common::Result<Self> {
        let projected_schema = match projection {
            Some(p) => Arc::new(table_schema.project(p)?),
            None => Arc::clone(table_schema),
        };

        let partition_col = Arc::new(Column::new(DATASET_MANIFEST_ID_FIELD_NAME, 0)) as Arc<dyn PhysicalExpr>;
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

        let mut physical_ordering = vec![PhysicalSortExpr::new(partition_col, SortOptions::new(false, true))];
        if let Some(col_expr) = order_col {
            physical_ordering.push(PhysicalSortExpr::new(col_expr, SortOptions::new(false, true)));
        }

        let orderings = vec![LexOrdering::new(physical_ordering)];

        let eq_properties =
            EquivalenceProperties::new_with_orderings(Arc::clone(&projected_schema), &orderings);

        let partition_in_output_schema = projection.map(|p| p.contains(&0)).unwrap_or(false);
        let num_partitions = DEFAULT_OUTPUT_PARTITIONS;

        let output_partitioning = if partition_in_output_schema {
            Partitioning::Hash(
                vec![Arc::new(Column::new(DATASET_MANIFEST_ID_FIELD_NAME, 0))],
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

        let chunk_info = compute_partition_stream_chunk_info(&chunk_info_batches)?;

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
            chunk_request,
        })
    }
}

async fn send_next_row(
    query_handle: &QueryHandle<StorageEngine>,
    partition_id: &str,
    target_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
) -> Result<Option<()>, DataFusionError> {
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

    let pid_array = Arc::new(StringArray::from(vec![
        partition_id.to_owned();
        next_row[0].len()
    ])) as Arc<dyn Array>;

    let mut arrays = Vec::with_capacity(num_fields + 1);
    arrays.push(pid_array);
    arrays.extend(next_row);

    let batch_schema = Arc::new(prepend_string_column_schema(
        &query_schema,
        DATASET_MANIFEST_ID_FIELD_NAME,
    ));

    let batch = RecordBatch::try_new(batch_schema, arrays)?;

    let output_batch = align_record_batch_to_schema(&batch, target_schema)?;

    output_channel
        .send(output_batch)
        .await
        .map_err(|err| exec_datafusion_err!("{err}"))?;

    Ok(Some(()))
}

async fn chunk_store_cpu_worker_thread(
    mut input_channel: Receiver<Vec<(Chunk, Option<String>)>>,
    output_channel: Sender<RecordBatch>,
    chunk_info: Arc<BTreeMap<String, Vec<ChunkInfo>>>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
) -> Result<(), DataFusionError> {

    let mut current_stores: Option<(
        String,
        ChunkStoreHandle,
        QueryHandle<StorageEngine>,
        Vec<ChunkInfo>,
    )> = None;
    while let Some(chunks_and_partition_ids) = input_channel.recv().await {
        for chunk_and_partition_id in chunks_and_partition_ids {
            let (chunk, partition_id) = chunk_and_partition_id;

            let partition_id = partition_id
                .ok_or_else(|| exec_datafusion_err!("Received chunk without a partition id"))?;

            if let Some((current_partition, _, query_handle, _)) = &current_stores {
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

            if current_stores.is_none() {
                let store_info = StoreInfo {
                    application_id: ApplicationId::from(partition_id.as_str()),
                    store_id: StoreId::random(StoreKind::Recording),
                    cloned_from: None,
                    store_source: StoreSource::Unknown,
                    store_version: None,
                };

                let mut store = ChunkStore::new(store_info.store_id.clone(), Default::default());
                store.set_store_info(store_info);
                let store = ChunkStoreHandle::new(store);

                let query_engine =
                    QueryEngine::new(store.clone(), QueryCache::new_handle(store.clone()));
                let query_handle = query_engine.query(query_expression.clone());

                let mut chunks_to_receive = chunk_info
                    .get(&partition_id)
                    .ok_or(exec_datafusion_err!(
                        "No chunk info for partition id {partition_id}"
                    ))?
                    .clone();
                chunks_to_receive.sort();

                current_stores =
                    Some((partition_id.clone(), store, query_handle, chunks_to_receive));
            };

            let (_, store, _, remaining_chunks) = &mut current_stores.as_mut().unwrap();

            let chunk_id = chunk.id();
            let Some((chunk_idx, _)) = remaining_chunks
                .iter()
                .enumerate()
                .find(|(_, info)| info.chunk_id == chunk_id)
            else {
                return exec_err!("Unable to locate chunk ID in expected return values");
            };

            store
                .write()
                .insert_chunk(&std::sync::Arc::new(chunk))
                .map_err(|err| exec_datafusion_err!("{err}"))?;

            // TODO(tsaucer) we should be able to send out intermediate rows as we are getting
            // data in, but the prior attempts to validate these were invalid
            remaining_chunks.remove(chunk_idx);
        }
    }

    // Flush out remaining of last partition
    if let Some((final_partition, _, query_handle, _)) =
        &mut current_stores.as_mut()
    {
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
/// which is different from the partition_id.
async fn chunk_stream_io_loop(
    mut client: ConnectionClient,
    base_request: GetChunksRequest,
    mut rerun_partition_ids: Vec<String>,
    output_channel: Sender<Vec<(Chunk, Option<String>)>>,
) -> Result<(), DataFusionError> {

    rerun_partition_ids.sort();
    for partition_id in rerun_partition_ids {
        let mut get_chunks_request = base_request.clone();
        get_chunks_request.partition_ids = vec![PartitionId::from(partition_id)];

        let get_chunks_response_stream = client
            .inner()
            .get_chunks(get_chunks_request)
            .instrument(tracing::trace_span!("chunk_stream_io_loop"))
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner();

        // Then we need to fully decode these chunks, i.e. both the transport layer (Protobuf)
        // and the app layer (Arrow).
        let mut chunk_stream = re_grpc_client::get_chunks_response_to_chunk_and_partition_id(
            get_chunks_response_stream,
        );

        // We want the underlying HTTP2 client to keep polling on the gRPC stream as fast
        // as non-blockingly possible, which cannot happen if we just poll once in a while
        // in-between decoding phases. This results in the stream just sleeping, waiting
        // for IO to complete, way more frequently that it should.
        // We resolve that by spawning a dedicated I/O task that just polls the stream as fast as
        // the stream will allows. This way, whenever the underlying HTTP2 stream is polled, we
        // will already have pre-fetched a bunch of data for it.
        while let Some(Ok(chunk_and_partition_id)) = chunk_stream.next().await {
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
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel(32); // 32 batches of chunks, not 32 chunks

        let random_state = ahash::RandomState::with_seeds(0, 0, 0, 0);
        let rerun_partition_ids = self
            .chunk_info
            .keys()
            .filter(|partition_id| {
                let hash_value = partition_id.hash_one(&random_state) as usize;
                hash_value % self.target_partitions == partition
            })
            .cloned()
            .collect::<Vec<_>>();

        let client = self.client.clone();
        let chunk_request = self.chunk_request.clone();

        let (batches_tx, batches_rx) = tokio::sync::mpsc::channel(32); // 32 batches of chunks, not 32 chunks
        let query_expression = self.query_expression.clone();
        let projected_schema = self.projected_schema.clone();
        let cpu_join_handle = Some(self.worker_runtime.handle().spawn(
            chunk_store_cpu_worker_thread(
                chunk_rx,
                batches_tx,
                Arc::clone(&self.chunk_info),
                query_expression,
                projected_schema,
            ),
        ));

        let stream = DataframePartitionStream {
            projected_schema: self.projected_schema.clone(),
            store_output_channel: batches_rx,
            client,
            chunk_request,
            rerun_partition_ids,
            chunk_tx: Some(chunk_tx),
            io_join_handle: None,
            cpu_join_handle,
            cpu_runtime: Arc::clone(&self.worker_runtime),
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
            chunk_request: self.chunk_request.clone(),
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
            "StreamingTableExec: num_partitions={:?}",
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
        // Notify the thread to shutdown.
        self.notify_shutdown.notify_one();
        // In a production system you also need to ensure your code stops adding
        // new tasks to the underlying runtime after this point to allow the
        // thread to complete its work and exit cleanly.
        if let Some(thread_join_handle) = self.thread_join_handle.take() {
            // If the thread is still running, we wait for it to finish
            if let Err(e) = thread_join_handle.join() {
                eprintln!("Error joining CPU runtime thread: {e:?}",);
            }
        }
    }
}

impl CpuRuntime {
    /// Create a new Tokio Runtime for CPU bound tasks
    pub fn try_new(num_threads: usize) -> Result<Self, DataFusionError> {
        let cpu_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_threads)
            .build()?;
        let handle = cpu_runtime.handle().clone();
        let notify_shutdown = Arc::new(Notify::new());
        let notify_shutdown_captured: Arc<Notify> = Arc::clone(&notify_shutdown);

        // The cpu_runtime runs and is dropped on a separate thread
        let thread_join_handle = std::thread::spawn(move || {
            cpu_runtime.block_on(async move {
                notify_shutdown_captured.notified().await;
            });
            // Note: cpu_runtime is dropped here, which will wait for all tasks
            // to complete
        });

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
