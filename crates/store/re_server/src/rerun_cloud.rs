use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ahash::HashMap;
use arrow::array::BinaryArray;
use arrow::record_batch::RecordBatch;
use datafusion::logical_expr::dml::InsertOp;
use datafusion::prelude::SessionContext;
use nohash_hasher::IntSet;
use tokio_stream::StreamExt as _;
use tonic::{Code, Request, Response, Status};

use re_chunk_store::{Chunk, ChunkStore, ChunkStoreHandle};
use re_log_encoding::ToTransport as _;
use re_log_types::{EntityPath, EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext::{LanceTable, ProviderDetails as _, TableInsertMode};
use re_protos::{
    cloud::v1alpha1::{
        DeleteEntryResponse, EntryDetails, EntryKind, FetchChunksRequest,
        GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse,
        GetDatasetSchemaResponse, GetPartitionTableSchemaResponse, QueryDatasetResponse,
        QueryTasksOnCompletionRequest, QueryTasksOnCompletionResponse, QueryTasksRequest,
        QueryTasksResponse, RegisterTableRequest, RegisterTableResponse,
        RegisterWithDatasetResponse, ScanDatasetManifestRequest, ScanDatasetManifestResponse,
        ScanPartitionTableResponse, ScanTableResponse,
        ext::{
            self, CreateDatasetEntryResponse, DataSource, ReadDatasetEntryResponse,
            ReadTableEntryResponse,
        },
        rerun_cloud_service_server::RerunCloudService,
    },
    common::v1alpha1::{
        TaskId,
        ext::{IfDuplicateBehavior, PartitionId},
    },
    headers::RerunHeadersExtractorExt as _,
};

use crate::entrypoint::NamedPath;
use crate::store::{ChunkKey, Dataset, InMemoryStore, Table};

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
        directory: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_dataset(directory, on_duplicate)?;

        Ok(self)
    }

    #[cfg(feature = "lance")]
    pub async fn with_directory_as_table(
        mut self,
        path: &NamedPath,
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

const DUMMY_TASK_ID: &str = "task_00000000DEADBEEF";

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

    /// Returns all the chunk stores of the specified dataset and partitions ids. If `partition_ids`
    /// is empty, return stores of all partitions.
    ///
    /// Returns (partition id, layer name, store) tuples.
    async fn get_chunk_stores(
        &self,
        dataset_id: EntryId,
        partition_ids: &[PartitionId],
    ) -> Result<Vec<(PartitionId, String, ChunkStoreHandle)>, tonic::Status> {
        let store = self.store.read().await;
        let dataset = store.dataset(dataset_id)?;

        Ok(dataset
            .partitions_from_ids(partition_ids)?
            .flat_map(|(partition_id, partition)| {
                partition.iter_layers().map(|(layer_name, layer)| {
                    (
                        partition_id.clone(),
                        layer_name.to_owned(),
                        layer.store_handle().clone(),
                    )
                })
            })
            .collect())
    }

    fn resolve_data_sources(data_sources: &[DataSource]) -> Result<Vec<DataSource>, tonic::Status> {
        let mut resolved = Vec::<DataSource>::with_capacity(data_sources.len());
        for source in data_sources {
            if source.is_prefix {
                let path = source.storage_url.to_file_path().map_err(|_err| {
                    tonic::Status::invalid_argument(format!(
                        "getting file path from {:?}",
                        source.storage_url
                    ))
                })?;
                let meta = std::fs::metadata(&path).map_err(|err| match err.kind() {
                    std::io::ErrorKind::NotFound => {
                        tonic::Status::invalid_argument(format!("Directory not found: {:?}", &path))
                    }
                    _ => tonic::Status::invalid_argument(format!(
                        "Failed to read directory metadata {path:?}: {err:#}"
                    )),
                })?;
                if !meta.is_dir() {
                    return Err(tonic::Status::invalid_argument(format!(
                        "Expected directory, got file: {path:?}"
                    )));
                }

                // Recursively walk the directory and grab all '.rrd' files
                let mut dirs_to_visit = vec![path];
                let mut files = Vec::new();

                while let Some(current_dir) = dirs_to_visit.pop() {
                    let entries = std::fs::read_dir(&current_dir).map_err(|err| {
                        tonic::Status::internal(format!(
                            "Failed to read directory {current_dir:?}: {err:#}"
                        ))
                    })?;

                    for entry in entries {
                        let entry = entry.map_err(|err| {
                            tonic::Status::internal(format!(
                                "Failed to read directory entry: {err:#}"
                            ))
                        })?;
                        let entry_path = entry.path();

                        if entry_path.is_dir() {
                            dirs_to_visit.push(entry_path);
                        } else if let Some(extension) = entry_path.extension()
                            && extension == "rrd"
                        {
                            files.push(entry_path);
                        }
                    }
                }

                for file_path in files {
                    let mut file_url = source.storage_url.clone();
                    file_url.set_path(&file_path.to_string_lossy());
                    resolved.push(DataSource {
                        storage_url: file_url,
                        is_prefix: false,
                        ..source.clone()
                    });
                }
            } else {
                resolved.push(source.clone());
            }
        }

        Ok(resolved)
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
decl_stream!(QueryDatasetResponseStream<manifest:QueryDatasetResponse>);
decl_stream!(ScanPartitionTableResponseStream<manifest:ScanPartitionTableResponse>);
decl_stream!(ScanDatasetManifestResponseStream<manifest:ScanDatasetManifestResponse>);
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

            (Some(entry_id), None) => Some(store.dataset(entry_id)?),

            (None, Some(name)) => Some(store.dataset_by_name(&name)?),

            (Some(entry_id), Some(name)) => {
                let dataset = store.dataset_by_name(&name)?;
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
        let dataset = store.dataset(entry_id)?;

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

        let data_sources = Self::resolve_data_sources(&data_sources)?;

        for source in data_sources {
            let ext::DataSource {
                storage_url,
                is_prefix,
                layer,
                kind,
            } = source;

            if is_prefix {
                return Err(tonic::Status::internal(
                    "register_with_dataset: prefix data sources should have been resolved already",
                ));
            }

            if kind != ext::DataSourceKind::Rrd {
                return Err(tonic::Status::unimplemented(
                    "register_with_dataset: only RRD data sources are implemented",
                ));
            }

            if let Ok(rrd_path) = storage_url.to_file_path() {
                let new_partition_ids = dataset.load_rrd(&rrd_path, Some(&layer), on_duplicate)?;

                for partition_id in new_partition_ids {
                    partition_ids.push(partition_id.to_string());
                    partition_layers.push(layer.clone());
                    partition_types.push("rrd".to_owned());
                    // TODO(RR-2289): this should probably be a memory address
                    storage_urls.push(storage_url.to_string());
                    task_ids.push(DUMMY_TASK_ID.to_owned());
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
                data: Some(record_batch.into()),
            },
        ))
    }

    // TODO(RR-2017): This endpoint is in need of a deep redesign. For now it defaults to
    // overwriting the "base" layer.
    async fn write_chunks(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteChunksRequest>>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::WriteChunksResponse>, tonic::Status>
    {
        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let mut request = request.into_inner();

        let mut chunk_stores = HashMap::default();

        while let Some(chunk_msg) = request.next().await {
            let chunk_msg = chunk_msg?;

            let chunk_batch: RecordBatch = chunk_msg
                .chunk
                .ok_or_else(|| tonic::Status::invalid_argument("no chunk in WriteChunksRequest"))?
                .try_into()
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

            chunk_stores
                .entry(partition_id.clone())
                .or_insert_with(|| {
                    ChunkStore::new(
                        StoreId::new(StoreKind::Recording, entry_id.to_string(), partition_id.id),
                        InMemoryStore::chunk_store_config(),
                    )
                })
                .insert_chunk(&chunk)
                .map_err(|err| {
                    tonic::Status::internal(format!("error adding chunk to store: {err:#}"))
                })?;
        }

        let mut store = self.store.write().await;
        let Some(dataset) = store.dataset_mut(entry_id) else {
            return Err(tonic::Status::not_found("dataset not found"));
        };

        #[expect(clippy::iter_over_hash_type)]
        for (entity_path, chunk_store) in chunk_stores {
            dataset.add_layer(
                entity_path,
                DataSource::DEFAULT_LAYER.to_owned(),
                ChunkStoreHandle::new(chunk_store),
                IfDuplicateBehavior::Error,
            )?;
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteChunksResponse {},
        ))
    }

    async fn write_table(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteTableRequest>>,
    ) -> Result<tonic::Response<re_protos::cloud::v1alpha1::WriteTableResponse>, tonic::Status>
    {
        // Limit the scope of the lock here to prevent deadlocks
        // when reading and writing to the same table
        let entry_id = {
            let store = self.store.read().await;
            get_entry_id_from_headers(&store, &request)?
        };

        let mut request = request.into_inner();

        while let Some(write_msg) = request.next().await {
            let write_msg = write_msg?;

            let rb = write_msg
                .dataframe_part
                .ok_or_else(|| {
                    tonic::Status::invalid_argument("no data frame in WriteTableRequest")
                })?
                .try_into()
                .map_err(|err| {
                    tonic::Status::internal(format!("Could not decode chunk: {err:#}"))
                })?;

            let mut store = self.store.write().await;
            let Some(table) = store.table_mut(entry_id) else {
                return Err(tonic::Status::not_found("table not found"));
            };
            let insert_op = match TableInsertMode::try_from(write_msg.insert_mode)
                .map_err(|err| Status::invalid_argument(err.to_string()))?
            {
                TableInsertMode::Append => InsertOp::Append,
                TableInsertMode::Overwrite => InsertOp::Overwrite,
            };

            table.write_table(rb, insert_op).await.map_err(|err| {
                tonic::Status::internal(format!("error writing to table: {err:#}"))
            })?;
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteTableResponse {},
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

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.partition_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read partition table: {err:#}"))
        })?;

        Ok(tonic::Response::new(GetPartitionTableSchemaResponse {
            schema: Some(
                record_batch
                    .schema_ref()
                    .as_ref()
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

        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.partition_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read partition table: {err:#}"))
        })?;

        let stream = futures::stream::once(async move {
            Ok(ScanPartitionTableResponse {
                data: Some(record_batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanPartitionTableStream
        ))
    }

    async fn get_dataset_manifest_schema(
        &self,
        request: Request<GetDatasetManifestSchemaRequest>,
    ) -> Result<Response<GetDatasetManifestSchemaResponse>, Status> {
        let store = self.store.read().await;

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.dataset_manifest()?;

        Ok(tonic::Response::new(GetDatasetManifestSchemaResponse {
            schema: Some(
                record_batch
                    .schema_ref()
                    .as_ref()
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!(
                            "unable to serialize Arrow schema: {err:#}"
                        ))
                    })?,
            ),
        }))
    }

    type ScanDatasetManifestStream = ScanDatasetManifestResponseStream;

    async fn scan_dataset_manifest(
        &self,
        request: Request<ScanDatasetManifestRequest>,
    ) -> Result<Response<Self::ScanDatasetManifestStream>, Status> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let request = request.into_inner();
        if !request.columns.is_empty() {
            return Err(tonic::Status::unimplemented(
                "scan_partition_table: column projection not implemented",
            ));
        }

        let dataset = store.dataset(entry_id)?;

        let record_batch = dataset.dataset_manifest()?;

        let stream = futures::stream::once(async move {
            Ok(ScanDatasetManifestResponse {
                data: Some(record_batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanDatasetManifestStream
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

        let dataset = store.dataset(entry_id)?;
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
            select_all_entity_paths,

            //TODO(RR-2613): we must do a much better job at handling these
            chunk_ids: _,
            fuzzy_descriptors: _,
            exclude_static_data: _,
            exclude_temporal_data: _,
            scan_parameters: _,
            query: _,
        } = request.into_inner();

        let entity_paths: IntSet<EntityPath> = entity_paths
            .into_iter()
            .map(EntityPath::try_from)
            .collect::<Result<IntSet<EntityPath>, _>>()?;
        if select_all_entity_paths && !entity_paths.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "cannot specify entity paths if `select_all_entity_paths` is true",
            ));
        }

        let partition_ids = partition_ids
            .into_iter()
            .map(PartitionId::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let chunk_stores = self.get_chunk_stores(entry_id, &partition_ids).await?;

        if chunk_stores.is_empty() {
            let stream = futures::stream::iter([{
                let batch = QueryDatasetResponse::create_empty_dataframe();
                let data = Some(batch.into());
                Ok(QueryDatasetResponse { data })
            }]);

            return Ok(tonic::Response::new(
                Box::pin(stream) as Self::QueryDatasetStream
            ));
        }

        let stream = futures::stream::iter(chunk_stores.into_iter().map(
            move |(partition_id, layer_name, store_handle)| {
                let num_chunks = store_handle.read().num_chunks();

                let mut chunk_ids = Vec::with_capacity(num_chunks);
                let mut chunk_partition_ids = Vec::with_capacity(num_chunks);
                let mut chunk_keys = Vec::with_capacity(num_chunks);
                let mut chunk_entity_path = Vec::with_capacity(num_chunks);
                let mut chunk_is_static = Vec::with_capacity(num_chunks);

                let mut timelines = BTreeMap::new();

                for chunk in store_handle.read().iter_chunks() {
                    if !entity_paths.is_empty() && !entity_paths.contains(chunk.entity_path()) {
                        continue;
                    }

                    let mut missing_timelines: BTreeSet<_> = timelines.keys().copied().collect();
                    for (timeline_name, timeline_col) in chunk.timelines() {
                        let range = timeline_col.time_range();
                        let time_min = range.min();
                        let time_max = range.max();

                        let timeline_name = timeline_name.as_str();
                        missing_timelines.remove(timeline_name);
                        let timeline_data_type = timeline_col.times_array().data_type().to_owned();

                        let timeline_data = timelines.entry(timeline_name).or_insert((
                            timeline_data_type,
                            vec![None; chunk_partition_ids.len()],
                            vec![None; chunk_partition_ids.len()],
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

                    chunk_partition_ids.push(partition_id.id.clone());
                    chunk_ids.push(chunk.id());
                    chunk_entity_path.push(chunk.entity_path().to_string());
                    chunk_is_static.push(chunk.is_static());
                    chunk_keys.push(
                        ChunkKey {
                            chunk_id: chunk.id(),
                            partition_id: partition_id.clone(),
                            layer_name: layer_name.clone(),
                            dataset_id: entry_id,
                        }
                        .encode()?,
                    );
                }

                let chunk_layer_names = vec![layer_name.clone(); chunk_ids.len()];
                let chunk_key_refs = chunk_keys.iter().map(|v| v.as_slice()).collect();
                let batch = QueryDatasetResponse::create_dataframe(
                    chunk_ids,
                    chunk_partition_ids,
                    chunk_layer_names,
                    chunk_key_refs,
                    chunk_entity_path,
                    chunk_is_static,
                )
                .map_err(|err| {
                    tonic::Status::internal(format!("Failed to create dataframe: {err:#}"))
                })?;

                let data = Some(batch.into());

                Ok(QueryDatasetResponse { data })
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::QueryDatasetStream
        ))
    }

    type FetchChunksStream = FetchChunksResponseStream;

    async fn fetch_chunks(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FetchChunksRequest>,
    ) -> std::result::Result<tonic::Response<Self::FetchChunksStream>, tonic::Status> {
        // worth noting that FetchChunks is not per-dataset request, it simply contains chunk infos
        let request = request.into_inner();

        let mut chunk_keys = vec![];
        for chunk_info_data in request.chunk_infos {
            let chunk_info_batch: RecordBatch = chunk_info_data.try_into().map_err(|err| {
                tonic::Status::internal(format!("Failed to decode chunk_info: {err:#}"))
            })?;

            let schema = chunk_info_batch.schema();

            let chunk_key_col_idx = schema
                .column_with_name(FetchChunksRequest::FIELD_CHUNK_KEY)
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "Missing {} column",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?
                .0;

            let chunk_keys_arr = chunk_info_batch
                .column(chunk_key_col_idx)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "{} must be binary array",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?;

            for chunk_key in chunk_keys_arr {
                let chunk_key = chunk_key.ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "{} must not be null",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?;

                let chunk_key = ChunkKey::decode(chunk_key)?;
                chunk_keys.push(chunk_key);
            }
        }

        let chunks = self
            .store
            .read()
            .await
            .chunks_from_chunk_keys(&chunk_keys)?;

        let compression = re_log_encoding::Compression::Off;

        let encoded_chunks = chunks
            .into_iter()
            .map(|(store_id, chunk)| {
                let arrow_msg = re_log_types::ArrowMsg {
                    chunk_id: *chunk.id(),
                    batch: chunk.to_record_batch().map_err(|err| {
                        tonic::Status::internal(format!(
                            "failed to convert chunk to record batch: {err:#}"
                        ))
                    })?,
                    on_release: None,
                };

                arrow_msg
                    .to_transport((store_id, compression))
                    .map_err(|err| tonic::Status::internal(format!("encoding failed: {err:#}")))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let response = re_protos::cloud::v1alpha1::FetchChunksResponse {
            chunks: encoded_chunks,
        };

        let stream = futures::stream::once(async move { Ok(response) });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::FetchChunksStream
        ))
    }

    // --- Table APIs ---

    async fn register_table(
        &self,
        request: tonic::Request<RegisterTableRequest>,
    ) -> Result<tonic::Response<RegisterTableResponse>, tonic::Status> {
        #[cfg_attr(not(feature = "lance"), expect(unused_mut))]
        let mut store = self.store.write().await;
        let request = request.into_inner();
        let Some(provider_details) = request.provider_details else {
            return Err(tonic::Status::invalid_argument("Missing provider details"));
        };
        #[cfg_attr(not(feature = "lance"), expect(unused_variables))]
        let lance_table = LanceTable::try_from_any(&provider_details)?
            .table_url
            .to_file_path()
            .map_err(|()| tonic::Status::invalid_argument("Invalid lance table path"))?;

        #[cfg(feature = "lance")]
        let entry_id = {
            let named_path = NamedPath {
                name: Some(request.name.clone()),
                path: lance_table,
            };

            store
                .load_directory_as_table(&named_path, IfDuplicateBehavior::Error)
                .await?
        };

        #[cfg(not(feature = "lance"))]
        let entry_id = EntryId::new();

        let table_entry = store
            .table(entry_id)
            .ok_or(Status::internal("table missing that was just registered"))?
            .as_table_entry();

        let response = RegisterTableResponse {
            table_entry: Some(table_entry.into()),
        };

        Ok(response.into())
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

        let schema = table.schema();

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
            let batch = batch.map_err(|err| tonic::Status::from_error(Box::new(err)))?;
            Ok(ScanTableResponse {
                dataframe_part: Some(batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(resp_stream) as Self::ScanTableStream
        ))
    }

    // --- Tasks service ---

    async fn query_tasks(
        &self,
        request: tonic::Request<QueryTasksRequest>,
    ) -> Result<tonic::Response<QueryTasksResponse>, tonic::Status> {
        let tasks_id = request.into_inner().ids;

        let dummy_task_id = TaskId {
            id: DUMMY_TASK_ID.to_owned(),
        };

        for task_id in &tasks_id {
            if task_id != &dummy_task_id {
                return Err(tonic::Status::not_found(format!(
                    "task {} not found",
                    task_id.id
                )));
            }
        }

        let rb = QueryTasksResponse::create_dataframe(
            vec![DUMMY_TASK_ID.to_owned()],
            vec![None],
            vec![None],
            vec!["success".to_owned()],
            vec![None],
            vec![None],
            vec![None],
            vec![None],
            vec![1],
            vec![None],
            vec![None],
        )
        .expect("constant content that should always succeed");

        // All tasks finish immediately in the OSS server
        Ok(tonic::Response::new(QueryTasksResponse {
            data: Some(rb.into()),
        }))
    }

    type QueryTasksOnCompletionStream = QueryTasksOnCompletionResponseStream;

    async fn query_tasks_on_completion(
        &self,
        request: tonic::Request<QueryTasksOnCompletionRequest>,
    ) -> Result<tonic::Response<Self::QueryTasksOnCompletionStream>, tonic::Status> {
        let task_ids = request.into_inner().ids;

        // All tasks finish immediately in the OSS server, so we can delegate to `query_tasks
        let response_data = self
            .query_tasks(tonic::Request::new(QueryTasksRequest { ids: task_ids }))
            .await?
            .into_inner()
            .data;

        Ok(tonic::Response::new(
            Box::pin(futures::stream::once(async move {
                Ok(QueryTasksOnCompletionResponse {
                    data: response_data,
                })
            })) as Self::QueryTasksOnCompletionStream,
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
fn get_entry_id_from_headers<T>(
    store: &InMemoryStore,
    req: &tonic::Request<T>,
) -> Result<EntryId, tonic::Status> {
    if let Some(entry_id) = req.entry_id()? {
        Ok(entry_id)
    } else if let Some(dataset_name) = req.entry_name()? {
        Ok(store.dataset_by_name(&dataset_name)?.id())
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
