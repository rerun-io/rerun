use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, DurationNanosecondArray, Int64Array, RecordBatch, RecordBatchOptions,
    StringArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use datafusion::prelude::SessionContext;
use nohash_hasher::IntSet;
use re_chunk_store::Chunk;
use re_chunk_store::external::re_chunk::external::re_byte_size::SizeBytes as _;
use re_entity_db::EntityDb;
use re_entity_db::external::re_query::StorageEngine;
use re_log_encoding::codec::wire::{decoder::Decode as _, encoder::Encode as _};
use re_log_types::external::re_types_core::{ChunkId, Loggable as _};
use re_log_types::{EntityPath, EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext::GetChunksRequest;
use re_protos::cloud::v1alpha1::{
    EntryDetails, GetChunksResponse, GetDatasetSchemaResponse, GetPartitionTableSchemaResponse,
    QueryDatasetResponse, ScanPartitionTableResponse, ScanTableResponse,
};
use re_protos::headers::RerunHeadersExtractorExt as _;
use re_protos::{cloud::v1alpha1::RegisterWithDatasetResponse, common::v1alpha1::ext::PartitionId};
use re_protos::{
    cloud::v1alpha1::ext,
    cloud::v1alpha1::ext::{
        CreateDatasetEntryResponse, ReadDatasetEntryResponse, ReadTableEntryResponse,
    },
};
use re_protos::{
    cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    cloud::v1alpha1::{
        FetchTaskOutputRequest, FetchTaskOutputResponse, QueryTasksOnCompletionRequest,
        QueryTasksRequest, QueryTasksResponse,
    },
};
use re_protos::{
    cloud::v1alpha1::{
        DeleteEntryResponse, EntryKind, RegisterTableRequest, RegisterTableResponse,
    },
    common::v1alpha1::ext::IfDuplicateBehavior,
};
use tokio_stream::StreamExt as _;
use tonic::{Code, Status};

use crate::store::{Dataset, InMemoryStore, Table};

#[derive(Debug, Default)]
pub struct RerunCloudHandlerSettings {}

#[derive(Default)]
pub struct RerunCloudHandlerBuilder {
    settings: RerunCloudHandlerSettings,

    store: InMemoryStore,
}

impl RerunCloudHandlerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_directory_as_dataset(
        mut self,
        directory: &std::path::Path,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_dataset(directory, on_duplicate)?;

        Ok(self)
    }

    pub async fn with_directory_as_table(
        mut self,
        path: &std::path::Path,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_table(path, on_duplicate)
            .await?;

        Ok(self)
    }

    pub fn build(self) -> RerunCloudHandler {
        RerunCloudHandler::new(self.settings, self.store)
    }
}

// ---

pub struct RerunCloudHandler {
    #[expect(dead_code)]
    settings: RerunCloudHandlerSettings,

    store: tokio::sync::RwLock<InMemoryStore>,
}

impl RerunCloudHandler {
    pub fn new(settings: RerunCloudHandlerSettings, store: InMemoryStore) -> Self {
        Self {
            settings,
            store: tokio::sync::RwLock::new(store),
        }
    }

    async fn get_storage_engines(
        &self,
        dataset_id: EntryId,
        mut partition_ids: Vec<PartitionId>,
    ) -> Result<Vec<(PartitionId, StorageEngine)>, tonic::Status> {
        let store = self.store.read().await;
        let dataset = store.dataset(dataset_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {dataset_id} not found"))
        })?;

        if partition_ids.is_empty() {
            partition_ids = dataset.partition_ids().collect();
        }

        partition_ids
            .into_iter()
            .map(|partition_id| {
                dataset
                    .partition(&partition_id)
                    .ok_or_else(|| {
                        tonic::Status::not_found(format!(
                            "Partition with ID {partition_id} not found"
                        ))
                    })
                    .map(|partition| {
                        #[expect(unsafe_code)]
                        // Safety: no viewer is running, and we've locked the store for the duration
                        // of the handler already.
                        unsafe { partition.storage_engine_raw() }.clone()
                    })
                    .map(|storage_engine| (partition_id, storage_engine))
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

impl std::fmt::Debug for RerunCloudHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RerunCloudHandler").finish()
    }
}

macro_rules! decl_stream {
    ($stream:ident<manifest:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<re_protos::cloud::v1alpha1::$resp, tonic::Status>>
                    + Send,
            >,
        >;
    };

    ($stream:ident<rerun_cloud:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<re_protos::cloud::v1alpha1::$resp, tonic::Status>>
                    + Send,
            >,
        >;
    };

    ($stream:ident<tasks:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<re_protos::cloud::v1alpha1::$resp, tonic::Status>>
                    + Send,
            >,
        >;
    };
}

decl_stream!(FetchChunksResponseStream<manifest:FetchChunksResponse>);
decl_stream!(GetChunksResponseStream<manifest:GetChunksResponse>);
decl_stream!(QueryDatasetResponseStream<manifest:QueryDatasetResponse>);
decl_stream!(ScanPartitionTableResponseStream<manifest:ScanPartitionTableResponse>);
decl_stream!(SearchDatasetResponseStream<manifest:SearchDatasetResponse>);
decl_stream!(ScanTableResponseStream<rerun_cloud:ScanTableResponse>);
decl_stream!(QueryTasksOnCompletionResponseStream<tasks:QueryTasksOnCompletionResponse>);

impl RerunCloudHandler {
    async fn find_datasets(
        &self,
        entry_id: Option<EntryId>,
        name: Option<String>,
    ) -> Result<Vec<EntryDetails>, Status> {
        let store = self.store.read().await;

        let dataset = match (entry_id, name) {
            (None, None) => None,

            (Some(entry_id), None) => {
                let Some(dataset) = store.dataset(entry_id) else {
                    return Err(tonic::Status::not_found(format!(
                        "Dataset with ID {entry_id} not found"
                    )));
                };
                Some(dataset)
            }

            (None, Some(name)) => {
                let Some(dataset) = store.dataset_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Dataset with name {name} not found"
                    )));
                };
                Some(dataset)
            }

            (Some(entry_id), Some(name)) => {
                let Some(dataset) = store.dataset_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Dataset with name {name} not found"
                    )));
                };
                if dataset.id() != entry_id {
                    return Err(tonic::Status::not_found(format!(
                        "Dataset with ID {entry_id} not found"
                    )));
                }
                Some(dataset)
            }
        };

        let dataset_iter = if let Some(dataset) = dataset {
            itertools::Either::Left(std::iter::once(dataset))
        } else {
            itertools::Either::Right(store.iter_datasets())
        };

        Ok(dataset_iter
            .map(Dataset::as_entry_details)
            .map(Into::into)
            .collect())
    }

    async fn find_tables(
        &self,
        entry_id: Option<EntryId>,
        name: Option<String>,
    ) -> Result<Vec<EntryDetails>, Status> {
        let store = self.store.read().await;

        let table = match (entry_id, name) {
            (None, None) => None,

            (Some(entry_id), None) => {
                let Some(table) = store.table(entry_id) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with ID {entry_id} not found"
                    )));
                };
                Some(table)
            }

            (None, Some(name)) => {
                let Some(table) = store.table_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with name {name} not found"
                    )));
                };
                Some(table)
            }

            (Some(entry_id), Some(name)) => {
                let Some(table) = store.table_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with name {name} not found"
                    )));
                };
                if table.id() != entry_id {
                    return Err(tonic::Status::not_found(format!(
                        "Table with ID {entry_id} not found"
                    )));
                }
                Some(table)
            }
        };

        let table_iter = if let Some(table) = table {
            itertools::Either::Left(std::iter::once(table))
        } else {
            itertools::Either::Right(store.iter_tables())
        };

        Ok(table_iter
            .map(Table::as_entry_details)
            .map(Into::into)
            .collect())
    }
}

#[tonic::async_trait]
impl RerunCloudService for RerunCloudHandler {
    async fn version(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::VersionRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::cloud::v1alpha1::VersionResponse>,
        tonic::Status,
    > {
        let re_protos::cloud::v1alpha1::VersionRequest {} = request.into_inner();

        // NOTE: Reminder that this is only fully filled iff CI=1.
        let build_info = re_build_info::build_info!();

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::VersionResponse {
                build_info: Some(build_info.into()),
            },
        ))
    }

    // --- Catalog ---

    async fn find_entries(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FindEntriesRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::FindEntriesResponse>, tonic::Status>
    {
        let filter = request.into_inner().filter;
        let entry_id = filter
            .as_ref()
            .and_then(|filter| filter.id)
            .map(TryInto::try_into)
            .transpose()?;
        let name = filter.as_ref().and_then(|filter| filter.name.clone());
        let kind = filter
            .and_then(|filter| filter.entry_kind)
            .map(EntryKind::try_from)
            .transpose()
            .map_err(|err| {
                Status::invalid_argument(format!("find_entries: invalid entry kind {err}"))
            })?;

        let entries = match kind {
            Some(EntryKind::Dataset) => self.find_datasets(entry_id, name).await?,
            Some(EntryKind::Table) => self.find_tables(entry_id, name).await?,
            None => {
                let mut datasets = match self.find_datasets(entry_id, name.clone()).await {
                    Ok(datasets) => datasets,
                    Err(err) => {
                        if err.code() == Code::NotFound {
                            vec![]
                        } else {
                            return Err(err);
                        }
                    }
                };
                let tables = match self.find_tables(entry_id, name).await {
                    Ok(tables) => tables,
                    Err(err) => {
                        if err.code() == Code::NotFound {
                            vec![]
                        } else {
                            return Err(err);
                        }
                    }
                };
                datasets.extend(tables);
                datasets
            }
            _ => {
                return Err(Status::unimplemented(
                    "find_entries: only datasets and tables are implemented",
                ));
            }
        };

        let response = re_protos::cloud::v1alpha1::FindEntriesResponse { entries };

        Ok(tonic::Response::new(response))
    }

    async fn create_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::CreateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::cloud::v1alpha1::CreateDatasetEntryResponse>,
        tonic::Status,
    > {
        let dataset_name: String = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        let dataset = store.create_dataset(&dataset_name).map_err(|err| {
            tonic::Status::internal(format!("Failed to create dataset entry: {err:#}"))
        })?;

        let dataset_entry = dataset.as_dataset_entry();

        Ok(tonic::Response::new(
            CreateDatasetEntryResponse {
                dataset: dataset_entry,
            }
            .into(),
        ))
    }

    async fn read_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadDatasetEntryRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::ReadDatasetEntryResponse>, tonic::Status>
    {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("entry with ID '{entry_id}' not found"))
        })?;

        Ok(tonic::Response::new(
            ReadDatasetEntryResponse {
                dataset_entry: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn update_dataset_entry(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::UpdateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::cloud::v1alpha1::UpdateDatasetEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "update_dataset_entry not implemented",
        ))
    }

    async fn read_table_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadTableEntryRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::cloud::v1alpha1::ReadTableEntryResponse>,
        tonic::Status,
    > {
        let store = self.store.read().await;

        let id = request
            .into_inner()
            .id
            .ok_or(Status::invalid_argument("No table entry ID provided"))?
            .try_into()?;

        let table = store.table(id).ok_or_else(|| {
            tonic::Status::not_found(format!("table with entry ID '{id}' not found"))
        })?;

        Ok(tonic::Response::new(
            ReadTableEntryResponse {
                table_entry: table.as_table_entry(),
            }
            .into(),
        ))
    }

    async fn delete_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::DeleteEntryRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::DeleteEntryResponse>, tonic::Status>
    {
        let entry_id = request.into_inner().try_into()?;

        self.store.write().await.delete_dataset(entry_id)?;

        Ok(tonic::Response::new(DeleteEntryResponse {}))
    }

    // --- Manifest Registry ---

    /* Write data */

    async fn register_with_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::RegisterWithDatasetRequest>,
    ) -> Result<
        tonic::Response<re_protos::cloud::v1alpha1::RegisterWithDatasetResponse>,
        tonic::Status,
    > {
        let mut store = self.store.write().await;
        let dataset_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset_mut(dataset_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Dataset with ID {dataset_id} not found"))
        })?;

        let re_protos::cloud::v1alpha1::ext::RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        } = request.into_inner().try_into()?;

        let mut partition_ids: Vec<String> = vec![];
        let mut partition_layers: Vec<String> = vec![];
        let mut partition_types: Vec<String> = vec![];
        let mut storage_urls: Vec<String> = vec![];
        let mut task_ids: Vec<String> = vec![];

        for source in data_sources {
            let ext::DataSource {
                storage_url,
                layer,
                kind,
            } = source;

            if layer != "base" {
                return Err(tonic::Status::unimplemented(format!(
                    "register_with_dataset: only 'base' layer is implemented, got {layer:?}"
                )));
            }

            if kind != ext::DataSourceKind::Rrd {
                return Err(tonic::Status::unimplemented(
                    "register_with_dataset: only RRD data sources are implemented",
                ));
            }

            if let Ok(rrd_path) = storage_url.to_file_path() {
                let new_partition_ids = dataset.load_rrd(&rrd_path, on_duplicate)?;

                for partition_id in new_partition_ids {
                    partition_ids.push(partition_id.to_string());
                    partition_layers.push(layer.clone());
                    partition_types.push("rrd".to_owned());
                    storage_urls.push(storage_url.to_string());
                    task_ids.push("<DUMMY TASK ID>".to_owned());
                }
            }
        }

        let record_batch = RegisterWithDatasetResponse::create_dataframe(
            partition_ids,
            partition_layers,
            partition_types,
            storage_urls,
            task_ids,
        )
        .map_err(|err| tonic::Status::internal(format!("Failed to create dataframe: {err:#}")))?;
        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::RegisterWithDatasetResponse {
                data: Some(record_batch.encode().map_err(|err| {
                    tonic::Status::internal(format!("Failed to encode dataframe: {err:#}"))
                })?),
            },
        ))
    }

    async fn write_chunks(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteChunksRequest>>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::WriteChunksResponse>, tonic::Status>
    {
        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let mut request = request.into_inner();

        let mut entity_dbs = HashMap::new();

        while let Some(chunk_msg) = request.next().await {
            let chunk_msg = chunk_msg?;

            let chunk_batch = chunk_msg
                .chunk
                .ok_or_else(|| tonic::Status::invalid_argument("no chunk in WriteChunksRequest"))?
                .decode()
                .map_err(|err| {
                    tonic::Status::internal(format!("Could not decode chunk: {err:#}"))
                })?;

            let partition_id: PartitionId = chunk_batch
                .schema()
                .metadata()
                .get("rerun:partition_id")
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(
                        "Received chunk without 'rerun.partition_id' metadata",
                    )
                })?
                .clone()
                .into();

            let chunk = Arc::new(Chunk::from_record_batch(&chunk_batch).map_err(|err| {
                tonic::Status::internal(format!("error decoding chunk from record batch: {err:#}"))
            })?);

            entity_dbs
                .entry(partition_id.clone())
                .or_insert_with(|| {
                    EntityDb::new(StoreId::new(
                        StoreKind::Recording,
                        entry_id.to_string(),
                        partition_id.id,
                    ))
                })
                .add_chunk(&chunk)
                .map_err(|err| {
                    tonic::Status::internal(format!("error adding chunk to store: {err:#}"))
                })?;
        }

        let mut store = self.store.write().await;
        let Some(dataset) = store.dataset_mut(entry_id) else {
            return Err(tonic::Status::not_found("dataset not found"));
        };

        #[expect(clippy::iter_over_hash_type)]
        for (entity_path, entity_db) in entity_dbs {
            dataset.add_partition(entity_path, entity_db);
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteChunksResponse {},
        ))
    }

    /* Query schemas */

    async fn get_partition_table_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetPartitionTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::cloud::v1alpha1::GetPartitionTableSchemaResponse>,
        tonic::Status,
    > {
        let store = self.store.read().await;

        // check that the dataset exists before returning
        _ = get_entry_id_from_headers(&store, &request)?;

        Ok(tonic::Response::new(GetPartitionTableSchemaResponse {
            schema: Some(
                (&ScanPartitionTableResponse::schema())
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!(
                            "unable to serialize Arrow schema: {err:#}"
                        ))
                    })?,
            ),
        }))
    }

    type ScanPartitionTableStream = ScanPartitionTableResponseStream;

    async fn scan_partition_table(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ScanPartitionTableRequest>,
    ) -> Result<tonic::Response<Self::ScanPartitionTableStream>, tonic::Status> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let request = request.into_inner();
        if !request.columns.is_empty() {
            return Err(tonic::Status::unimplemented(
                "scan_partition_table: column projection not implemented",
            ));
        }

        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        let record_batch = dataset.partition_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read partition table: {err:#}"))
        })?;

        let stream = futures::stream::once(async move {
            record_batch
                .encode()
                .map(|data| ScanPartitionTableResponse { data: Some(data) })
                .map_err(|err| {
                    tonic::Status::internal(format!("failed encoding metadata: {err:#}"))
                })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanPartitionTableStream
        ))
    }

    async fn get_dataset_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetDatasetSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::cloud::v1alpha1::GetDatasetSchemaResponse>,
        tonic::Status,
    > {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        let schema = dataset.schema().map_err(|err| {
            tonic::Status::internal(format!("Unable to read dataset schema: {err:#}"))
        })?;

        Ok(tonic::Response::new(GetDatasetSchemaResponse {
            schema: Some((&schema).try_into().map_err(|err| {
                tonic::Status::internal(format!("Unable to serialize Arrow schema: {err:#}"))
            })?),
        }))
    }

    /* Indexing */

    async fn create_index(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::CreateIndexRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::cloud::v1alpha1::CreateIndexResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("create_index not implemented"))
    }

    /* Queries */

    type SearchDatasetStream = SearchDatasetResponseStream;

    async fn search_dataset(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::SearchDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::SearchDatasetStream>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "search_dataset not implemented",
        ))
    }

    type QueryDatasetStream = QueryDatasetResponseStream;

    async fn query_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::QueryDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::QueryDatasetStream>, tonic::Status> {
        if !request.get_ref().chunk_ids.is_empty() {
            return Err(tonic::Status::unimplemented(
                "query_dataset: querying specific chunk ids is not implemented",
            ));
        }

        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let re_protos::cloud::v1alpha1::QueryDatasetRequest {
            partition_ids,
            entity_paths,
            ..
        } = request.into_inner();

        let entity_paths: IntSet<EntityPath> = entity_paths
            .into_iter()
            .map(EntityPath::try_from)
            .collect::<Result<IntSet<EntityPath>, _>>()?;

        let partition_ids = partition_ids
            .into_iter()
            .map(PartitionId::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let storage_engines = self.get_storage_engines(entry_id, partition_ids).await?;

        let stream = futures::stream::iter(storage_engines.into_iter().map(
            move |(partition_id, storage_engine)| {
                let storage_read = storage_engine.read();
                let chunk_store = storage_read.store();
                let num_rows = chunk_store.num_chunks();

                let mut chunk_partition_id = Vec::with_capacity(num_rows);
                let mut chunk_entity_path = Vec::with_capacity(num_rows);
                let mut chunk_id = Vec::with_capacity(num_rows);
                let mut chunk_is_static = Vec::with_capacity(num_rows);
                let mut chunk_byte_len = Vec::with_capacity(num_rows);

                let mut timelines = BTreeMap::new();

                chunk_store
                    .iter_chunks()
                    .filter(|chunk| {
                        entity_paths.is_empty() || entity_paths.contains(chunk.entity_path())
                    })
                    .for_each(|chunk| {
                        let mut missing_timelines: BTreeSet<_> =
                            timelines.keys().copied().collect();
                        for (timeline_name, timeline_col) in chunk.timelines() {
                            let range = timeline_col.time_range();
                            let time_min = range.min();
                            let time_max = range.max();

                            let timeline_name = timeline_name.as_str();
                            missing_timelines.remove(timeline_name);
                            let timeline_data_type =
                                timeline_col.times_array().data_type().to_owned();

                            let timeline_data = timelines.entry(timeline_name).or_insert((
                                timeline_data_type,
                                vec![None; chunk_partition_id.len()],
                                vec![None; chunk_partition_id.len()],
                            ));

                            timeline_data.1.push(Some(time_min.as_i64()));
                            timeline_data.2.push(Some(time_max.as_i64()));
                        }
                        for timeline_name in missing_timelines {
                            let timeline_data = timelines
                                .get_mut(timeline_name)
                                .expect("timeline_names already checked"); // Already checked

                            timeline_data.1.push(None);
                            timeline_data.2.push(None);
                        }

                        chunk_partition_id.push(partition_id.id.clone());
                        chunk_entity_path.push(chunk.entity_path().to_owned());
                        chunk_id.push(chunk.id());
                        chunk_is_static.push(chunk.is_static());
                        chunk_byte_len.push(chunk.heap_size_bytes());
                    });

                // The output schema of `query_dataset` contains information about
                // the chunks such as the start and end times of each timeline.
                // We will need to compute the schema based on which indices exist
                // in the store.
                let mut output_fields = vec![
                    Field::new(
                        "chunk_partition_id",
                        arrow::datatypes::DataType::Utf8,
                        false,
                    )
                    .with_metadata(
                        std::iter::once(("rerun:kind".to_owned(), "control".to_owned())).collect(),
                    ),
                    Field::new("chunk_entity_path", arrow::datatypes::DataType::Utf8, false)
                        .with_metadata(
                            std::iter::once(("rerun:kind".to_owned(), "control".to_owned()))
                                .collect(),
                        ),
                    Field::new("chunk_id", ChunkId::arrow_datatype(), false).with_metadata(
                        std::iter::once(("rerun:kind".to_owned(), "control".to_owned())).collect(),
                    ),
                    Field::new(
                        "chunk_is_static",
                        arrow::datatypes::DataType::Boolean,
                        false,
                    )
                    .with_metadata(
                        std::iter::once(("rerun:kind".to_owned(), "control".to_owned())).collect(),
                    ),
                    Field::new("chunk_byte_len", arrow::datatypes::DataType::UInt64, false),
                ];
                let row_count = chunk_partition_id.len();
                let mut arrays = vec![
                    Arc::new(StringArray::from(chunk_partition_id)) as ArrayRef,
                    EntityPath::to_arrow(chunk_entity_path).map_err(|err| {
                        tonic::Status::internal(format!("EntityPath to_arrow failed: {err:#}"))
                    })? as ArrayRef,
                    ChunkId::to_arrow(chunk_id).map_err(|err| {
                        tonic::Status::internal(format!("ChunkId to_arrow failed: {err:#}"))
                    })? as ArrayRef,
                    Arc::new(BooleanArray::from(chunk_is_static)) as ArrayRef,
                    Arc::new(UInt64Array::from(chunk_byte_len)) as ArrayRef,
                ];

                for (timeline_name, (data_type, starts, ends)) in timelines {
                    let (starts, ends) = arrays_from_timelines(&data_type, starts, ends)
                        .map_err(tonic::Status::internal)?;

                    output_fields.push(Field::new(
                        format!("{timeline_name}:start"),
                        starts.data_type().to_owned(),
                        true,
                    ));
                    output_fields.push(Field::new(
                        format!("{timeline_name}:end"),
                        ends.data_type().to_owned(),
                        true,
                    ));

                    arrays.push(starts);
                    arrays.push(ends);
                }

                let schema = Arc::new(Schema::new_with_metadata(output_fields, HashMap::default()));
                let batch = RecordBatch::try_new_with_options(
                    schema,
                    arrays,
                    &RecordBatchOptions::default().with_row_count(Some(row_count)),
                )
                .map_err(|err| {
                    tonic::Status::internal(format!("record batch creation failed: {err:#}"))
                })?;

                let data =
                    Some(batch.encode().map_err(|err| {
                        tonic::Status::internal(format!("encoding failed: {err:#}"))
                    })?);

                Ok(QueryDatasetResponse { data })
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::QueryDatasetStream
        ))
    }

    type GetChunksStream = GetChunksResponseStream;

    async fn get_chunks(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetChunksRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetChunksStream>, tonic::Status> {
        let GetChunksRequest {
            dataset_id,
            partition_ids,
            chunk_ids,
            entity_paths,

            // We don't support queries, so you always get everything
            query: _,
        } = GetChunksRequest::try_from(request.into_inner())?;

        if !chunk_ids.is_empty() {
            return Err(tonic::Status::unimplemented(
                "get_chunks: querying specific chunk ids is not implemented",
            ));
        }

        let entity_paths: IntSet<EntityPath> = entity_paths.into_iter().collect();

        let storage_engines = self.get_storage_engines(dataset_id, partition_ids).await?;

        let stream = futures::stream::iter(storage_engines.into_iter().map(
            move |(partition_id, storage_engine)| {
                let compression = re_log_encoding::Compression::Off;
                let store_id = StoreId::new(
                    StoreKind::Recording,
                    dataset_id.to_string(),
                    partition_id.id.as_str(),
                );

                let arrow_msgs: Result<Vec<_>, _> = storage_engine
                    // NOTE: ⚠️This is super cursed ⚠️The underlying lock is synchronous: the only
                    // reason this doesn't deadlock is because we collect() at the end of this mapping,
                    // before the overarching stream ever gets a chance to yield.
                    // Make sure it stays that way.
                    .read()
                    .store()
                    .iter_chunks()
                    .filter(|chunk| {
                        entity_paths.is_empty() || entity_paths.contains(chunk.entity_path())
                    })
                    .map(|chunk| {
                        let arrow_msg = re_log_types::ArrowMsg {
                            chunk_id: *chunk.id(),
                            batch: chunk.to_record_batch()?,
                            on_release: None,
                        };

                        re_log_encoding::protobuf_conversions::arrow_msg_to_proto(
                            &arrow_msg,
                            store_id.clone(),
                            compression,
                        )
                    })
                    .collect();

                Ok(GetChunksResponse {
                    chunks: arrow_msgs.map_err(|err| {
                        tonic::Status::internal(format!("encoding failed: {err:#}"))
                    })?,
                })
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::GetChunksStream
        ))
    }

    type FetchChunksStream = FetchChunksResponseStream;

    async fn fetch_chunks(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FetchChunksRequest>,
    ) -> std::result::Result<tonic::Response<Self::FetchChunksStream>, tonic::Status> {
        // worth noting that FetchChunks is not per-dataset request, it simply contains chunk infos
        let request = request.into_inner();

        let mut chunk_partition_pairs = Vec::new();

        for chunk_info_data in request.chunk_infos {
            let chunk_info_batch = chunk_info_data.decode().map_err(|err| {
                tonic::Status::internal(format!("Failed to decode chunk_info: {err:#}"))
            })?;

            let schema = chunk_info_batch.schema();
            let chunk_id_col = schema
                .column_with_name("chunk_id")
                .ok_or_else(|| tonic::Status::invalid_argument("Missing chunk_id column"))?;
            let partition_id_col = schema
                .column_with_name("chunk_partition_id")
                .or_else(|| schema.column_with_name("partition_id"))
                .ok_or_else(|| tonic::Status::invalid_argument("Missing partition_id column"))?;

            let chunk_ids = chunk_info_batch.column(chunk_id_col.0);
            let partition_ids = chunk_info_batch
                .column(partition_id_col.0)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    tonic::Status::invalid_argument("partition_id must be string array")
                })?;

            use re_log_types::external::re_types_core::ChunkId;
            let chunk_id_array = ChunkId::from_arrow(chunk_ids).map_err(|err| {
                tonic::Status::internal(format!("Failed to parse chunk_id column: {err:#}"))
            })?;

            for (i, chunk_id) in chunk_id_array.into_iter().enumerate() {
                if let Some(partition_id_str) = partition_ids.value(i).to_owned().into() {
                    let partition_id = PartitionId::from(partition_id_str);
                    chunk_partition_pairs.push((chunk_id, partition_id));
                }
            }
        }

        // get storage engines only for the partitions we actually need
        let store = self.store.read().await;
        let storage_engines: std::collections::HashMap<_, _> = store
            .iter_datasets()
            .flat_map(|dataset| {
                let dataset_id = dataset.id();
                let chunk_partition_pairs = &chunk_partition_pairs;
                dataset.partition_ids().filter_map(move |partition_id| {
                    if chunk_partition_pairs
                        .iter()
                        .any(|(_, pid)| pid == &partition_id)
                    {
                        dataset.partition(&partition_id).map(|partition| {
                            #[expect(unsafe_code)]
                            // Safety: no viewer is running, and we've locked the store for the duration
                            // of the handler already.
                            let storage_engine = unsafe { partition.storage_engine_raw() }.clone();
                            (partition_id, (dataset_id, storage_engine))
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();
        drop(store);

        let mut chunks = Vec::new();
        let compression = re_log_encoding::Compression::Off;

        for (chunk_id, partition_id) in chunk_partition_pairs {
            let (dataset_id, storage_engine) =
                storage_engines.get(&partition_id).ok_or_else(|| {
                    tonic::Status::internal(format!(
                        "Storage engine not found for partition {partition_id}"
                    ))
                })?;

            let storage_read = storage_engine.read();
            let chunk_store = storage_read.store();

            if let Some(chunk) = chunk_store.chunk(&chunk_id) {
                let store_id = StoreId::new(
                    StoreKind::Recording,
                    dataset_id.to_string(),
                    partition_id.id.as_str(),
                );

                let arrow_msg = re_log_types::ArrowMsg {
                    chunk_id: *chunk.id(),
                    batch: chunk.to_record_batch().map_err(|err| {
                        tonic::Status::internal(format!(
                            "failed to convert chunk to record batch: {err:#}"
                        ))
                    })?,
                    on_release: None,
                };

                let proto_msg = re_log_encoding::protobuf_conversions::arrow_msg_to_proto(
                    &arrow_msg,
                    store_id,
                    compression,
                )
                .map_err(|err| tonic::Status::internal(format!("encoding failed: {err:#}")))?;

                chunks.push(proto_msg);
            }
        }

        let response = re_protos::cloud::v1alpha1::FetchChunksResponse { chunks };

        let stream = futures::stream::once(async move { Ok(response) });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::FetchChunksStream
        ))
    }

    // --- Table APIs ---

    async fn register_table(
        &self,
        _request: tonic::Request<RegisterTableRequest>,
    ) -> Result<tonic::Response<RegisterTableResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "register_table not implemented",
        ))
    }

    async fn get_table_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetTableSchemaRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::GetTableSchemaResponse>, Status> {
        let store = self.store.read().await;
        let Some(entry_id) = request.into_inner().table_id else {
            return Err(Status::not_found("Table ID not specified in request"));
        };
        let entry_id = entry_id.try_into()?;

        let table = store
            .table(entry_id)
            .ok_or_else(|| Status::not_found(format!("Entry with ID {entry_id} not found")))?;

        let lance_table = table.provider();

        let schema = lance_table.schema();

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::GetTableSchemaResponse {
                schema: Some(schema.as_ref().try_into().map_err(|err| {
                    Status::internal(format!("Unable to serialize Arrow schema: {err:#}"))
                })?),
            },
        ))
    }

    type ScanTableStream = ScanTableResponseStream;

    async fn scan_table(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ScanTableRequest>,
    ) -> Result<tonic::Response<Self::ScanTableStream>, Status> {
        let store = self.store.read().await;
        let Some(entry_id) = request.into_inner().table_id else {
            return Err(Status::not_found("Table ID not specified in request"));
        };
        let entry_id = entry_id.try_into()?;

        let table = store
            .table(entry_id)
            .ok_or_else(|| Status::not_found(format!("Entry with ID {entry_id} not found")))?;

        let ctx = SessionContext::default();
        let plan = table
            .provider()
            .scan(&ctx.state(), None, &[], None)
            .await
            .map_err(|err| Status::internal(format!("failed to scan table: {err:#}")))?;

        let stream = plan
            .execute(0, ctx.task_ctx())
            .map_err(|err| tonic::Status::from_error(Box::new(err)))?;

        let resp_stream = stream.map(|batch| {
            batch
                .map_err(|err| tonic::Status::from_error(Box::new(err)))?
                .encode()
                .map(|batch| ScanTableResponse {
                    dataframe_part: Some(batch),
                })
                .map_err(|err| tonic::Status::internal(format!("Error encoding chunk: {err:#}")))
        });

        Ok(tonic::Response::new(
            Box::pin(resp_stream) as Self::ScanTableStream
        ))
    }

    // --- Tasks service ---

    async fn query_tasks(
        &self,
        _request: tonic::Request<QueryTasksRequest>,
    ) -> Result<tonic::Response<QueryTasksResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("query_tasks not implemented"))
    }

    type QueryTasksOnCompletionStream = QueryTasksOnCompletionResponseStream;

    async fn query_tasks_on_completion(
        &self,
        _request: tonic::Request<QueryTasksOnCompletionRequest>,
    ) -> Result<tonic::Response<Self::QueryTasksOnCompletionStream>, tonic::Status> {
        // All tasks finish emmidiately in the OSS server
        Ok(tonic::Response::new(
            Box::pin(futures::stream::empty()) as Self::QueryTasksOnCompletionStream
        ))
    }

    async fn fetch_task_output(
        &self,
        _request: tonic::Request<FetchTaskOutputRequest>,
    ) -> Result<tonic::Response<FetchTaskOutputResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "fetch_task_output not implemented",
        ))
    }

    async fn update_entry(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::UpdateEntryRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateEntryResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented("update_entry not implemented"))
    }

    async fn do_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoMaintenanceRequest>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::DoMaintenanceResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented(
            "do_maintenance not implemented",
        ))
    }

    async fn do_global_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest>,
    ) -> Result<
        tonic::Response<re_protos::cloud::v1alpha1::DoGlobalMaintenanceResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "do_global_maintenance not implemented",
        ))
    }
}

/// Retrieves the entry ID based on HTTP headers.
#[expect(clippy::result_large_err)] // it's just a tonic::Status
fn get_entry_id_from_headers<T>(
    store: &InMemoryStore,
    req: &tonic::Request<T>,
) -> Result<EntryId, tonic::Status> {
    if let Some(entry_id) = req.entry_id()? {
        Ok(entry_id)
    } else if let Some(dataset_name) = req.entry_name()? {
        Ok(store
            .dataset_by_name(&dataset_name)
            .ok_or_else(|| {
                tonic::Status::not_found(format!("entry with name '{dataset_name}' not found"))
            })?
            .id())
    } else {
        const HEADERS: &[&str] = &[
            re_protos::headers::RERUN_HTTP_HEADER_ENTRY_ID,
            re_protos::headers::RERUN_HTTP_HEADER_ENTRY_NAME,
        ];
        Err(tonic::Status::invalid_argument(format!(
            "missing mandatory {HEADERS:?} HTTP headers"
        )))
    }
}

fn arrays_from_timelines(
    data_type: &DataType,
    starts: Vec<Option<i64>>,
    ends: Vec<Option<i64>>,
) -> Result<(ArrayRef, ArrayRef), &'static str> {
    let (starts, ends) = match data_type {
        DataType::Int64 => (
            Arc::new(Int64Array::from(starts)) as ArrayRef,
            Arc::new(Int64Array::from(ends)) as ArrayRef,
        ),
        // downcast_value!(time_array, Int64Array).reinterpret_cast::<Int64Type>(),
        DataType::Timestamp(TimeUnit::Second, _) => (
            Arc::new(TimestampSecondArray::from(starts)) as ArrayRef,
            Arc::new(TimestampSecondArray::from(ends)) as ArrayRef,
        ),
        DataType::Timestamp(TimeUnit::Millisecond, _) => (
            Arc::new(TimestampMillisecondArray::from(starts)) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(ends)) as ArrayRef,
        ),
        DataType::Timestamp(TimeUnit::Microsecond, _) => (
            Arc::new(TimestampMicrosecondArray::from(starts)) as ArrayRef,
            Arc::new(TimestampMicrosecondArray::from(ends)) as ArrayRef,
        ),
        DataType::Timestamp(TimeUnit::Nanosecond, _) => (
            Arc::new(TimestampNanosecondArray::from(starts)) as ArrayRef,
            Arc::new(TimestampNanosecondArray::from(ends)) as ArrayRef,
        ),
        DataType::Duration(TimeUnit::Nanosecond) => (
            Arc::new(DurationNanosecondArray::from(starts)) as ArrayRef,
            Arc::new(DurationNanosecondArray::from(ends)) as ArrayRef,
        ),
        _ => {
            return Err("Unexpected timeline data type for index");
        }
    };

    Ok((starts, ends))
}
