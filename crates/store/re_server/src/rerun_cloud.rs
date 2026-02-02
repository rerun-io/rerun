use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use ahash::HashMap;
use arrow::array::BinaryArray;
use arrow::record_batch::RecordBatch;
use cfg_if::cfg_if;
use datafusion::logical_expr::dml::InsertOp;
use datafusion::prelude::SessionContext;
use nohash_hasher::IntSet;
use tokio_stream::StreamExt as _;
use tonic::{Code, Request, Response, Status};

use re_arrow_util::RecordBatchExt as _;
use re_chunk_store::{
    Chunk, ChunkStore, ChunkStoreHandle, LatestAtQuery, OnMissingChunk, RangeQuery,
};
use re_log_encoding::ToTransport as _;
use re_log_types::{EntityPath, EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext::LanceTable;
use re_protos::cloud::v1alpha1::ext::{
    self, CreateDatasetEntryRequest, CreateDatasetEntryResponse, CreateTableEntryRequest,
    CreateTableEntryResponse, DataSource, EntryDetailsUpdate, ProviderDetails, QueryDatasetRequest,
    ReadDatasetEntryResponse, ReadTableEntryResponse, TableInsertMode, UpdateDatasetEntryRequest,
    UpdateDatasetEntryResponse, UpdateEntryRequest, UpdateEntryResponse,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    DeleteEntryResponse, EntryDetails, EntryKind, FetchChunksRequest,
    GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse, GetDatasetSchemaResponse,
    GetRrdManifestResponse, GetSegmentTableSchemaResponse, QueryDatasetResponse,
    QueryTasksOnCompletionRequest, QueryTasksOnCompletionResponse, QueryTasksRequest,
    QueryTasksResponse, RegisterTableRequest, RegisterTableResponse, RegisterWithDatasetResponse,
    ScanDatasetManifestRequest, ScanDatasetManifestResponse, ScanSegmentTableResponse,
    ScanTableResponse,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_protos::headers::RerunHeadersExtractorExt as _;
use re_protos::missing_field;
use re_tuid::Tuid;

use crate::OnError;
use crate::entrypoint::NamedPath;
use crate::store::{ChunkKey, Dataset, Error, InMemoryStore, TASK_ID_SUCCESS, Table, TaskResult};

#[derive(Debug)]
pub struct RerunCloudHandlerSettings {
    storage_dir: tempfile::TempDir,
}

impl Default for RerunCloudHandlerSettings {
    fn default() -> Self {
        Self {
            storage_dir: create_data_dir().expect("Failed to create data directory"),
        }
    }
}

fn create_data_dir() -> Result<tempfile::TempDir, crate::store::Error> {
    Ok(tempfile::Builder::new().prefix("rerun-data-").tempdir()?)
}

#[derive(Default)]
pub struct RerunCloudHandlerBuilder {
    settings: RerunCloudHandlerSettings,
    store: InMemoryStore,
}

impl RerunCloudHandlerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn with_directory_as_dataset(
        mut self,
        directory: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
        on_error: crate::OnError,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_dataset(directory, on_duplicate, on_error)
            .await?;

        Ok(self)
    }

    pub async fn with_rrds_as_dataset(
        mut self,
        dataset_name: String,
        rrd_paths: Vec<PathBuf>,
        on_duplicate: IfDuplicateBehavior,
        on_error: crate::OnError,
    ) -> Result<Self, crate::store::Error> {
        let dataset = self.store.create_dataset(dataset_name, None)?;

        for rrd_path in rrd_paths {
            if let Err(err) = dataset
                .load_rrd(&rrd_path, None, on_duplicate, StoreKind::Recording)
                .await
            {
                match on_error {
                    OnError::Continue => {
                        re_log::warn!("Failed loading file {}: {err}", rrd_path.display());
                    }
                    OnError::Abort => {
                        return Err(err);
                    }
                }
            }
        }

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

pub struct RerunCloudHandler {
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

    /// Returns all the chunk stores of the specified dataset and segment ids. If `segment_ids`
    /// is empty, return stores of all segments.
    ///
    /// Returns (segment id, layer name, store) tuples.
    async fn get_chunk_stores(
        &self,
        dataset_id: EntryId,
        segment_ids: &[SegmentId],
    ) -> tonic::Result<Vec<(SegmentId, String, ChunkStoreHandle)>> {
        let store = self.store.read().await;
        let dataset = store.dataset(dataset_id)?;

        Ok(dataset
            .segments_from_ids(segment_ids)?
            .flat_map(|(segment_id, segment)| {
                segment.iter_layers().map(|(layer_name, layer)| {
                    (
                        segment_id.clone(),
                        layer_name.to_owned(),
                        layer.store_handle().clone(),
                    )
                })
            })
            .collect())
    }

    fn resolve_data_sources(data_sources: &[DataSource]) -> tonic::Result<Vec<DataSource>> {
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
                        "expected prefix / directory but got an object ({path:?})"
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

                if files.is_empty() {
                    return Err(tonic::Status::invalid_argument(format!(
                        "no rrd files found in {:?}",
                        source.storage_url
                    )));
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
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };

    ($stream:ident<rerun_cloud:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };

    ($stream:ident<tasks:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };
}

decl_stream!(FetchChunksResponseStream<manifest:FetchChunksResponse>);
decl_stream!(GetRrdManifestResponseStream<manifest:GetRrdManifestResponse>);
decl_stream!(QueryDatasetResponseStream<manifest:QueryDatasetResponse>);
decl_stream!(QueryTasksOnCompletionResponseStream<tasks:QueryTasksOnCompletionResponse>);
decl_stream!(ScanDatasetManifestResponseStream<manifest:ScanDatasetManifestResponse>);
decl_stream!(ScanSegmentTableResponseStream<manifest:ScanSegmentTableResponse>);
decl_stream!(ScanTableResponseStream<rerun_cloud:ScanTableResponse>);
decl_stream!(SearchDatasetResponseStream<manifest:SearchDatasetResponse>);

impl RerunCloudHandler {
    async fn find_datasets(
        &self,
        entry_id: Option<EntryId>,
        name: Option<String>,
        store_kind: Option<StoreKind>,
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
            .filter(|dataset| {
                store_kind.is_none_or(|store_kind| dataset.store_kind() == store_kind)
            })
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
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::VersionResponse>> {
        let re_protos::cloud::v1alpha1::VersionRequest {} = request.into_inner();

        // NOTE: Reminder that this is only fully filled iff CI=1.
        let build_info = re_build_info::build_info!();

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::VersionResponse {
                build_info: Some(build_info.into()),
                version: re_build_info::exposed_version!().to_owned(),
            },
        ))
    }

    // --- Catalog ---

    async fn find_entries(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FindEntriesRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::FindEntriesResponse>> {
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
            Some(EntryKind::Dataset) => {
                self.find_datasets(entry_id, name, Some(StoreKind::Recording))
                    .await?
            }

            Some(EntryKind::BlueprintDataset) => {
                self.find_datasets(entry_id, name, Some(StoreKind::Blueprint))
                    .await?
            }

            Some(EntryKind::Table) => self.find_tables(entry_id, name).await?,

            Some(EntryKind::DatasetView | EntryKind::TableView) => {
                return Err(Status::unimplemented(
                    "find_entries: dataset and table views are not supported",
                ));
            }

            Some(EntryKind::Unspecified) => {
                return Err(Status::invalid_argument(
                    "find_entries: entry kind unspecified",
                ));
            }

            None => {
                let mut datasets = match self.find_datasets(entry_id, name.clone(), None).await {
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
        };

        let response = re_protos::cloud::v1alpha1::FindEntriesResponse { entries };

        Ok(tonic::Response::new(response))
    }

    async fn create_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::CreateDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::CreateDatasetEntryResponse>>
    {
        let CreateDatasetEntryRequest {
            name: dataset_name,
            id: dataset_id,
        } = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        let dataset = store.create_dataset(dataset_name, dataset_id)?;

        Ok(tonic::Response::new(
            CreateDatasetEntryResponse {
                dataset: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn read_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::ReadDatasetEntryResponse>> {
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
        request: tonic::Request<re_protos::cloud::v1alpha1::UpdateDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateDatasetEntryResponse>>
    {
        let request: UpdateDatasetEntryRequest = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        let dataset = store.dataset_mut(request.id)?;

        dataset.set_dataset_details(request.dataset_details);

        Ok(tonic::Response::new(
            UpdateDatasetEntryResponse {
                dataset_entry: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn read_table_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadTableEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::ReadTableEntryResponse>> {
        let store = self.store.read().await;

        let id = request
            .into_inner()
            .id
            .ok_or_else(|| Status::invalid_argument("No table entry ID provided"))?
            .try_into()?;

        let table = store.table(id).ok_or_else(|| {
            tonic::Status::not_found(format!("table with entry ID '{id}' not found"))
        })?;

        Ok(tonic::Response::new(
            ReadTableEntryResponse {
                table_entry: table.as_table_entry(),
            }
            .try_into()?,
        ))
    }

    async fn delete_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::DeleteEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DeleteEntryResponse>> {
        let entry_id = request.into_inner().try_into()?;

        self.store.write().await.delete_entry(entry_id)?;

        Ok(tonic::Response::new(DeleteEntryResponse {}))
    }

    async fn update_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::UpdateEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateEntryResponse>> {
        let UpdateEntryRequest {
            id: entry_id,
            entry_details_update: EntryDetailsUpdate { name },
        } = request.into_inner().try_into()?;

        let mut store = self.store.write().await;

        if let Some(name) = name {
            store.rename_entry(entry_id, name)?;
        }

        Ok(tonic::Response::new(
            UpdateEntryResponse {
                entry_details: store.entry_details(entry_id)?,
            }
            .into(),
        ))
    }

    // --- Manifest Registry ---

    async fn register_with_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::RegisterWithDatasetRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::RegisterWithDatasetResponse>>
    {
        let mut store = self.store.write().await;
        let dataset_id = get_entry_id_from_headers(&store, &request)?;

        let ext::RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        } = request.into_inner().try_into()?;

        let mut segment_ids: Vec<String> = vec![];
        let mut segment_layers: Vec<String> = vec![];
        let mut segment_types: Vec<String> = vec![];
        let mut storage_urls: Vec<String> = vec![];
        let mut task_ids: Vec<String> = vec![];

        // Collect task results to register after all dataset operations complete
        let mut failed_task_results: Vec<(TaskId, TaskResult)> = vec![];

        let data_sources = Self::resolve_data_sources(&data_sources)?;
        if data_sources.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "no data sources to register",
            ));
        }

        // Process data sources within a block to limit the mutable borrow of dataset
        {
            let dataset = store.dataset_mut(dataset_id)?;

            for source in data_sources {
                let ext::DataSource {
                    storage_url,
                    is_prefix,
                    layer,
                    kind,
                } = source;

                // TODO(ab): Should some or all of these errors be returned as task error instead?
                // (No point in doing so unless this is tested in re_redap_tests.)
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
                    if !rrd_path.exists() {
                        return Err(tonic::Status::not_found(format!(
                            "RRD file not found, file does not exists: {rrd_path:?}"
                        )));
                    }

                    if !rrd_path.is_file() {
                        return Err(tonic::Status::not_found(format!(
                            "RRD file not found, path is not a file: {rrd_path:?}"
                        )));
                    }

                    // Try to load the RRD, capturing schema conflicts as task failures
                    let load_result = dataset
                        .load_rrd(&rrd_path, Some(&layer), on_duplicate, dataset.store_kind())
                        .await;

                    match load_result {
                        Ok(new_segment_ids) => {
                            for segment_id in new_segment_ids {
                                segment_ids.push(segment_id.to_string());
                                segment_layers.push(layer.clone());
                                segment_types.push("rrd".to_owned());
                                storage_urls.push(storage_url.to_string());

                                task_ids.push(TASK_ID_SUCCESS.to_owned());
                            }
                        }

                        Err(Error::SchemaConflict(msg)) => {
                            // In that case, we capture the failure in the returned tasks, but do
                            // not fail the rpc call.
                            // Generate a unique task ID for this data source

                            segment_ids.push(String::new());
                            segment_layers.push(layer.clone());
                            segment_types.push("rrd".to_owned());
                            storage_urls.push(storage_url.to_string());

                            let task_id = TaskId::new();
                            task_ids.push(task_id.id.clone());
                            failed_task_results.push((task_id, TaskResult::failed(&msg)));
                        }

                        Err(other_err) => {
                            // For other errors, still fail the RPC
                            return Err(other_err.into());
                        }
                    }
                } else {
                    return if storage_url.scheme() == "file" && storage_url.host().is_some() {
                        Err(tonic::Status::not_found(format!(
                            "RRD file not found, file URI should not have a host: {storage_url} (this may be caused by invalid relative-path URI)"
                        )))
                    } else {
                        Err(tonic::Status::not_found(format!(
                            "RRD file not found, could not load URI: {storage_url}"
                        )))
                    };
                }
            }
        }

        // Register all task results now that the mutable borrow of dataset is done
        for (task_id, result) in failed_task_results {
            store.task_registry().register_failure(task_id, result);
        }

        let record_batch = RegisterWithDatasetResponse::create_dataframe(
            segment_ids,
            segment_layers,
            segment_types,
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
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::WriteChunksResponse>> {
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

            // Support both new "rerun:segment_id" and legacy "rerun:partition_id" keys
            let schema = chunk_batch.schema();
            let metadata = schema.metadata();
            let segment_id: SegmentId = metadata
                .get("rerun:segment_id")
                .or_else(|| metadata.get("rerun:partition_id"))
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(
                        "Received chunk without 'rerun:segment_id' metadata",
                    )
                })?
                .clone()
                .into();

            let chunk = Arc::new(Chunk::from_record_batch(&chunk_batch).map_err(|err| {
                tonic::Status::internal(format!("error decoding chunk from record batch: {err:#}"))
            })?);

            chunk_stores
                .entry(segment_id.clone())
                .or_insert_with(|| {
                    ChunkStore::new(
                        StoreId::new(
                            StoreKind::Recording,
                            entry_id.to_string(),
                            segment_id.id.clone(),
                        ),
                        InMemoryStore::chunk_store_config(),
                    )
                })
                .insert_chunk(&chunk)
                .map_err(|err| {
                    tonic::Status::internal(format!("error adding chunk to store: {err:#}"))
                })?;
        }

        let mut store = self.store.write().await;
        let dataset = store.dataset_mut(entry_id)?;

        #[expect(clippy::iter_over_hash_type)]
        for (entity_path, chunk_store) in chunk_stores {
            dataset
                .add_layer(
                    entity_path,
                    DataSource::DEFAULT_LAYER.to_owned(),
                    ChunkStoreHandle::new(chunk_store),
                    IfDuplicateBehavior::Error,
                )
                .await?;
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteChunksResponse {},
        ))
    }

    async fn write_table(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteTableRequest>>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::WriteTableResponse>> {
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
                TableInsertMode::Replace => InsertOp::Replace,
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

    async fn get_segment_table_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetSegmentTableSchemaRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::GetSegmentTableSchemaResponse>>
    {
        let store = self.store.read().await;

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.segment_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read segment table: {err:#}"))
        })?;

        Ok(tonic::Response::new(GetSegmentTableSchemaResponse {
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

    type ScanSegmentTableStream = ScanSegmentTableResponseStream;

    async fn scan_segment_table(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ScanSegmentTableRequest>,
    ) -> tonic::Result<tonic::Response<Self::ScanSegmentTableStream>> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let request = request.into_inner();

        let dataset = store.dataset(entry_id)?;
        let mut record_batch = dataset.segment_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read segment table: {err:#}"))
        })?;

        // project columns
        if !request.columns.is_empty() {
            record_batch = record_batch
                .project_columns(request.columns.iter().map(|s| s.as_str()))
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!("Unable to project columns: {err:#}"))
                })?;
        }

        let stream = futures::stream::once(async move {
            Ok(ScanSegmentTableResponse {
                data: Some(record_batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanSegmentTableStream
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

        let dataset = store.dataset(entry_id)?;
        let mut record_batch = dataset.dataset_manifest()?;

        // project columns
        if !request.columns.is_empty() {
            record_batch = record_batch
                .project_columns(request.columns.iter().map(|s| s.as_str()))
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!("Unable to project columns: {err:#}"))
                })?;
        }

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
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::GetDatasetSchemaResponse>> {
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

    type GetRrdManifestStream = GetRrdManifestResponseStream;

    async fn get_rrd_manifest(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetRrdManifestRequest>,
    ) -> tonic::Result<tonic::Response<Self::GetRrdManifestStream>> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let request = request.into_inner();
        let segment_id = request
            .segment_id
            .ok_or_else(|| {
                missing_field!(
                    re_protos::cloud::v1alpha1::GetRrdManifestRequest,
                    "segment_id"
                )
            })?
            .try_into()?;

        let dataset = store.dataset(entry_id)?;
        let rrd_manifest = dataset.rrd_manifest(&segment_id)?;

        let rrd_manifest_stream =
            futures::stream::once(futures::future::ok(GetRrdManifestResponse {
                rrd_manifest: Some(rrd_manifest.to_transport(()).map_err(|err| {
                    tonic::Status::internal(format!("Unable to compute RRD manifest: {err:#}"))
                })?),
            }));

        Ok(tonic::Response::new(
            Box::pin(rrd_manifest_stream) as Self::GetRrdManifestStream
        ))
    }

    /* Indexing */

    async fn create_index(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::CreateIndexRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::CreateIndexResponse>> {
        cfg_if! {
            if #[cfg(feature = "lance")] {
                let store = self.store.read().await;
                let entry_id = get_entry_id_from_headers(&store, &request)?;
                let dataset = store.dataset(entry_id)?;

                dataset.indexes().create_index(dataset, request.into_inner().try_into()?).await
            } else {
                let _ = request;
                Err(tonic::Status::unimplemented("create_index requires the `lance` feature"))
            }
        }
    }

    async fn list_indexes(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ListIndexesRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::ListIndexesResponse>> {
        cfg_if! {
            if #[cfg(feature = "lance")] {
                let store = self.store.read().await;
                let entry_id = get_entry_id_from_headers(&store, &request)?;
                let dataset = store.dataset(entry_id)?;

                dataset.indexes().list_indexes(request.into_inner()).await
            } else {
                let _ = request;
                Err(tonic::Status::unimplemented("list_indexes requires the `lance` feature"))
            }
        }
    }

    async fn delete_indexes(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::DeleteIndexesRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DeleteIndexesResponse>> {
        cfg_if! {
            if #[cfg(feature = "lance")] {
                let store = self.store.read().await;
                let entry_id = get_entry_id_from_headers(&store, &request)?;
                let dataset = store.dataset(entry_id)?;

                let request = request.into_inner();
                let column = request.column.ok_or_else(|| {
                    missing_field!(re_protos::cloud::v1alpha1::DeleteIndexesRequest, "column")
                })?;

                dataset.indexes().delete_indexes(column.try_into()?).await
            } else {
                let _ = request;
                Err(tonic::Status::unimplemented("delete_indexes requires the `lance` feature"))
            }
        }
    }

    /* Queries */

    type SearchDatasetStream = SearchDatasetResponseStream;

    async fn search_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::SearchDatasetRequest>,
    ) -> tonic::Result<tonic::Response<Self::SearchDatasetStream>> {
        cfg_if! {
            if #[cfg(feature = "lance")] {
                let store = self.store.read().await;
                let entry_id = get_entry_id_from_headers(&store, &request)?;
                let dataset = store.dataset(entry_id)?;

                Ok(crate::chunk_index::DatasetChunkIndexes::search_dataset(dataset, request.into_inner().try_into()?).await?)
            } else {
                let _ = request;
                Err(tonic::Status::unimplemented("search_dataset requires the `lance` feature"))
            }
        }
    }

    type QueryDatasetStream = QueryDatasetResponseStream;

    async fn query_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::QueryDatasetRequest>,
    ) -> tonic::Result<tonic::Response<Self::QueryDatasetStream>> {
        if !request.get_ref().chunk_ids.is_empty() {
            return Err(tonic::Status::unimplemented(
                "query_dataset: querying specific chunk ids is not implemented",
            ));
        }

        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let QueryDatasetRequest {
            segment_ids,
            entity_paths,
            select_all_entity_paths,

            //TODO(RR-2613): we must do a much better job at handling these
            chunk_ids: requested_chunk_ids,
            fuzzy_descriptors: _,
            exclude_static_data,
            exclude_temporal_data,
            scan_parameters,
            query,
        } = request.into_inner().try_into()?;

        if scan_parameters.is_some() {
            // Logged at a low debug-level, because of https://github.com/rerun-io/rerun/pull/12578
            re_log::debug_once!("   scan_parameters are not yet implemented and will be ignored");
        }

        let entity_paths: IntSet<EntityPath> = entity_paths.into_iter().collect();
        if select_all_entity_paths && !entity_paths.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "cannot specify entity paths if `select_all_entity_paths` is true",
            ));
        }

        let chunk_stores = self.get_chunk_stores(entry_id, &segment_ids).await?;

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
            move |(segment_id, layer_name, store_handle)| {
                let num_chunks = store_handle.read().num_physical_chunks();

                let mut chunk_ids = Vec::with_capacity(num_chunks);
                let mut chunk_segment_ids = Vec::with_capacity(num_chunks);
                let mut chunk_keys = Vec::with_capacity(num_chunks);
                let mut chunk_entity_path = Vec::with_capacity(num_chunks);
                let mut chunk_is_static = Vec::with_capacity(num_chunks);
                let mut chunk_byte_sizes = Vec::with_capacity(num_chunks);

                let mut timelines = BTreeMap::new();

                let chunks = if let Some(query) = &query {
                    get_chunks_for_query(&store_handle, &entity_paths, query)
                } else {
                    store_handle
                        .read()
                        .iter_physical_chunks()
                        .map(Clone::clone)
                        .collect()
                };

                for chunk in chunks {
                    if !entity_paths.is_empty() && !entity_paths.contains(chunk.entity_path()) {
                        continue;
                    }

                    if !requested_chunk_ids.is_empty() && !requested_chunk_ids.contains(&chunk.id())
                    {
                        continue;
                    }

                    // Filter by static/temporal data
                    if exclude_static_data && chunk.is_static() {
                        continue;
                    }
                    if exclude_temporal_data && !chunk.is_static() {
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

                        let timeline_data = timelines.entry(timeline_name).or_insert_with(|| {
                            (
                                timeline_data_type,
                                vec![None; chunk_segment_ids.len()],
                                vec![None; chunk_segment_ids.len()],
                            )
                        });

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

                    chunk_segment_ids.push(segment_id.id.clone());
                    chunk_ids.push(chunk.id());
                    chunk_entity_path.push(chunk.entity_path().to_string());
                    chunk_is_static.push(chunk.is_static());

                    // Calculate chunk byte size for batching optimization
                    let chunk_size_bytes =
                        re_byte_size::SizeBytes::total_size_bytes(chunk.as_ref());
                    chunk_byte_sizes.push(chunk_size_bytes);

                    chunk_keys.push(
                        ChunkKey {
                            chunk_id: chunk.id(),
                            segment_id: segment_id.clone(),
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
                    chunk_segment_ids,
                    chunk_layer_names,
                    chunk_key_refs,
                    chunk_entity_path,
                    chunk_is_static,
                    chunk_byte_sizes,
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
    ) -> tonic::Result<tonic::Response<Self::FetchChunksStream>> {
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

        let stream = futures::stream::iter(chunks).map(|(store_id, chunk)| {
            let arrow_msg = re_log_types::ArrowMsg {
                chunk_id: *chunk.id(),
                batch: chunk.to_record_batch().map_err(|err| {
                    tonic::Status::internal(format!(
                        "failed to convert chunk to record batch: {err:#}"
                    ))
                })?,
                on_release: None,
            };

            let compression = re_log_encoding::Compression::Off;

            let encoded_chunk = arrow_msg
                .to_transport((store_id, compression))
                .map_err(|err| tonic::Status::internal(format!("encoding failed: {err:#}")))?;

            Ok(re_protos::cloud::v1alpha1::FetchChunksResponse {
                chunks: vec![encoded_chunk],
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::FetchChunksStream
        ))
    }

    // --- Table APIs ---

    async fn register_table(
        &self,
        request: tonic::Request<RegisterTableRequest>,
    ) -> tonic::Result<tonic::Response<RegisterTableResponse>> {
        #[cfg_attr(not(feature = "lance"), expect(unused_mut))]
        let mut store = self.store.write().await;
        let request = request.into_inner();
        let Some(provider_details) = request.provider_details else {
            return Err(tonic::Status::invalid_argument("Missing provider details"));
        };
        #[cfg_attr(not(feature = "lance"), expect(unused_variables))]
        let lance_table = match ProviderDetails::try_from(&provider_details) {
            Ok(ProviderDetails::LanceTable(lance_table)) => lance_table.table_url,
            Ok(ProviderDetails::SystemTable(_)) => Err(Status::invalid_argument(
                "System tables cannot be registered",
            ))?,
            Err(err) => return Err(err.into()),
        }
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
            .ok_or_else(|| Status::internal("table missing that was just registered"))?
            .as_table_entry();

        let response = RegisterTableResponse {
            table_entry: Some(table_entry.try_into()?),
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
    ) -> tonic::Result<tonic::Response<QueryTasksResponse>> {
        let task_ids = request.into_inner().ids;
        let store = self.store.read().await;

        let mut ids = Vec::with_capacity(task_ids.len());
        let mut exec_statuses = Vec::with_capacity(task_ids.len());
        let mut msgs = Vec::with_capacity(task_ids.len());

        for task_id in task_ids {
            // Look up the task in the registry, falling back to success for unknown IDs
            // (including legacy dummy IDs and stale task IDs)
            let result = store
                .task_registry()
                .get(&task_id)
                .unwrap_or_else(TaskResult::success);

            ids.push(task_id.id);
            exec_statuses.push(result.exec_status);
            msgs.push(if result.msgs.is_empty() {
                None
            } else {
                Some(result.msgs)
            });
        }

        let num_tasks = ids.len();
        let rb = QueryTasksResponse::create_dataframe(
            ids,
            vec![None; num_tasks], // kind
            vec![None; num_tasks], // data
            exec_statuses,
            msgs,
            vec![None; num_tasks], // blob_len
            vec![None; num_tasks], // lease_owner
            vec![None; num_tasks], // lease_expiration
            vec![1; num_tasks],    // attempts
            vec![None; num_tasks], // creation_time
            vec![None; num_tasks], // last_update_time
        )
        .map_err(|err| tonic::Status::internal(format!("Failed to create dataframe: {err:#}")))?;

        // All tasks finish immediately in the OSS server
        Ok(tonic::Response::new(QueryTasksResponse {
            data: Some(rb.into()),
        }))
    }

    type QueryTasksOnCompletionStream = QueryTasksOnCompletionResponseStream;

    async fn query_tasks_on_completion(
        &self,
        request: tonic::Request<QueryTasksOnCompletionRequest>,
    ) -> tonic::Result<tonic::Response<Self::QueryTasksOnCompletionStream>> {
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

    async fn do_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoMaintenanceRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DoMaintenanceResponse>> {
        Err(tonic::Status::unimplemented(
            "do_maintenance not implemented",
        ))
    }

    async fn do_global_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DoGlobalMaintenanceResponse>>
    {
        Err(tonic::Status::unimplemented(
            "do_global_maintenance not implemented",
        ))
    }

    async fn create_table_entry(
        &self,
        request: Request<re_protos::cloud::v1alpha1::CreateTableEntryRequest>,
    ) -> Result<Response<re_protos::cloud::v1alpha1::CreateTableEntryResponse>, Status> {
        let mut store = self.store.write().await;

        let request: CreateTableEntryRequest = request.into_inner().try_into()?;
        let table_name = &request.name;

        let schema = Arc::new(request.schema);

        let details = if let Some(details) = request.provider_details {
            details
        } else {
            // Create a directory in the storage directory. We use a tuid to avoid collisions
            // and avoid any sanitization issue with the provided table name.
            let table_path = self
                .settings
                .storage_dir
                .path()
                .join(format!("lance-{}", Tuid::new()));
            ProviderDetails::LanceTable(LanceTable {
                table_url: url::Url::from_directory_path(table_path).map_err(|_err| {
                    Status::internal(format!(
                        "Failed to create table directory in {:?}",
                        self.settings.storage_dir.path()
                    ))
                })?,
            })
        };

        let table = match details {
            ProviderDetails::LanceTable(table) => {
                store
                    .create_table_entry(table_name, &table.table_url, schema)
                    .await?
            }
            ProviderDetails::SystemTable(_) => {
                return Err(tonic::Status::invalid_argument(
                    "Creating system tables is not supported",
                ));
            }
        };

        Ok(Response::new(
            CreateTableEntryResponse { table }.try_into()?,
        ))
    }
}

/// Retrieves the entry ID based on HTTP headers.
fn get_entry_id_from_headers<T>(
    store: &InMemoryStore,
    req: &tonic::Request<T>,
) -> tonic::Result<EntryId> {
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

/// Return the equivalent latest at query
fn latest_at_or_static(latest_at: &ext::QueryLatestAt) -> LatestAtQuery {
    match &latest_at.index {
        Some(index) => LatestAtQuery::new(index.clone().into(), latest_at.at),
        None => {
            // Static only data
            LatestAtQuery::new("".into(), re_log_types::TimeInt::MIN)
        }
    }
}

/// Utility function to determine the chunks to return based on query parameters
fn get_chunks_for_query(
    store_handle: &ChunkStoreHandle,
    entity_paths: &IntSet<EntityPath>,
    query: &ext::Query,
) -> Vec<Arc<Chunk>> {
    let paths = if entity_paths.is_empty() {
        store_handle.read().all_entities()
    } else {
        entity_paths.clone()
    };
    match (&query.latest_at, &query.range) {
        (Some(latest_at), Some(range)) => {
            let latest_at = latest_at_or_static(latest_at);
            let range = RangeQuery::new(range.index.clone().into(), range.index_range);

            // We have both a latest at and a range, so we need to combine
            // chunks and ensure no duplicates
            paths
                .iter()
                .flat_map(|entity_path| {
                    let read_lock = store_handle.read();
                    let mut latest_at = read_lock
                        .latest_at_relevant_chunks_for_all_components(
                            OnMissingChunk::Report,
                            &latest_at,
                            entity_path,
                            true,
                        )
                        .chunks;
                    let mut range = read_lock
                        .range_relevant_chunks_for_all_components(
                            OnMissingChunk::Report,
                            &range.clone(),
                            entity_path,
                            true,
                        )
                        .chunks;

                    range.retain(|chunk| !latest_at.contains(chunk));
                    latest_at.extend(range);

                    latest_at
                })
                .collect::<Vec<_>>()
        }
        (Some(latest_at), None) => {
            let latest_at = latest_at_or_static(latest_at);

            paths
                .iter()
                .flat_map(|entity_path| {
                    store_handle
                        .read()
                        .latest_at_relevant_chunks_for_all_components(
                            OnMissingChunk::Report,
                            &latest_at.clone(),
                            entity_path,
                            true,
                        )
                        .chunks
                })
                .collect::<Vec<_>>()
        }
        (None, Some(range)) => {
            let range = RangeQuery::new(range.index.clone().into(), range.index_range);
            paths
                .iter()
                .flat_map(|entity_path| {
                    store_handle
                        .read()
                        .range_relevant_chunks_for_all_components(
                            OnMissingChunk::Report,
                            &range.clone(),
                            entity_path,
                            true,
                        )
                        .chunks
                })
                .collect::<Vec<_>>()
        }
        (None, None) => store_handle
            .read()
            .iter_physical_chunks()
            .map(Clone::clone)
            .collect(),
    }
}
