use std::collections::HashMap;
use std::sync::Arc;

use nohash_hasher::IntSet;
use tokio_stream::StreamExt as _;

use re_chunk_store::Chunk;
use re_entity_db::EntityDb;
use re_log_encoding::codec::wire::{decoder::Decode as _, encoder::Encode as _};
use re_log_types::{EntityPath, EntryId, StoreId, StoreKind};
use re_protos::catalog::v1alpha1::ext::{CreateDatasetEntryResponse, ReadDatasetEntryResponse};
use re_protos::catalog::v1alpha1::{
    DeleteEntryResponse, EntryKind, RegisterTableRequest, RegisterTableResponse,
};
use re_protos::common::v1alpha1::ext::PartitionId;
use re_protos::frontend::v1alpha1::ext::{GetChunksRequest, ScanPartitionTableRequest};
use re_protos::manifest_registry::v1alpha1::{
    GetChunksResponse, GetDatasetSchemaResponse, GetPartitionTableSchemaResponse,
    ScanPartitionTableResponse,
};
use re_protos::{
    frontend::v1alpha1::frontend_service_server::FrontendService,
    redap_tasks::v1alpha1::{
        FetchTaskOutputRequest, FetchTaskOutputResponse, QueryTasksOnCompletionRequest,
        QueryTasksRequest, QueryTasksResponse,
    },
};

use crate::store::{Dataset, InMemoryStore};

#[derive(Debug, Default)]
pub struct FrontendHandlerSettings {}

#[derive(Default)]
pub struct FrontendHandlerBuilder {
    settings: FrontendHandlerSettings,

    store: InMemoryStore,
}

impl FrontendHandlerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_directory_as_dataset(
        mut self,
        directory: &std::path::Path,
    ) -> Result<Self, crate::store::Error> {
        self.store.load_directory_as_dataset(directory)?;

        Ok(self)
    }

    pub fn build(self) -> FrontendHandler {
        FrontendHandler::new(self.settings, self.store)
    }
}

// ---

pub struct FrontendHandler {
    #[expect(dead_code)]
    settings: FrontendHandlerSettings,

    store: tokio::sync::RwLock<InMemoryStore>,
}

impl FrontendHandler {
    pub fn new(settings: FrontendHandlerSettings, store: InMemoryStore) -> Self {
        Self {
            settings,
            store: tokio::sync::RwLock::new(store),
        }
    }
}

impl std::fmt::Debug for FrontendHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontendHandler").finish()
    }
}

macro_rules! decl_stream {
    ($stream:ident<manifest:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<
                        Item = Result<re_protos::manifest_registry::v1alpha1::$resp, tonic::Status>,
                    > + Send,
            >,
        >;
    };

    ($stream:ident<frontend:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<
                        Item = Result<re_protos::frontend::v1alpha1::$resp, tonic::Status>,
                    > + Send,
            >,
        >;
    };

    ($stream:ident<tasks:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<
                        Item = Result<re_protos::redap_tasks::v1alpha1::$resp, tonic::Status>,
                    > + Send,
            >,
        >;
    };
}

decl_stream!(GetChunksResponseStream<manifest:GetChunksResponse>);
decl_stream!(QueryDatasetResponseStream<manifest:QueryDatasetResponse>);
decl_stream!(ScanPartitionTableResponseStream<manifest:ScanPartitionTableResponse>);
decl_stream!(SearchDatasetResponseStream<manifest:SearchDatasetResponse>);
decl_stream!(ScanTableResponseStream<frontend:ScanTableResponse>);
decl_stream!(QueryTasksOnCompletionResponseStream<tasks:QueryTasksOnCompletionResponse>);

#[tonic::async_trait]
impl FrontendService for FrontendHandler {
    async fn version(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::VersionRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::frontend::v1alpha1::VersionResponse>,
        tonic::Status,
    > {
        let re_protos::frontend::v1alpha1::VersionRequest {} = request.into_inner();

        // NOTE: Reminder that this is only fully filled iff CI=1.
        let build_info = re_build_info::build_info!();

        Ok(tonic::Response::new(
            re_protos::frontend::v1alpha1::VersionResponse {
                build_info: Some(build_info.into()),
            },
        ))
    }

    // --- Catalog ---

    async fn find_entries(
        &self,
        request: tonic::Request<re_protos::catalog::v1alpha1::FindEntriesRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::FindEntriesResponse>, tonic::Status>
    {
        let filter = request.into_inner().filter;
        let entry_id = filter
            .as_ref()
            .and_then(|filter| filter.id)
            .map(TryInto::try_into)
            .transpose()?;
        let name = filter.as_ref().and_then(|filter| filter.name.clone());
        let kind = filter.and_then(|filter| filter.entry_kind);

        if kind.is_some_and(|kind| kind != EntryKind::Dataset as i32) {
            return Err(tonic::Status::unimplemented(
                "find_entries: only datasets are implemented",
            ));
        }

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

        let response = re_protos::catalog::v1alpha1::FindEntriesResponse {
            entries: dataset_iter
                .map(Dataset::as_entry_details)
                .map(Into::into)
                .collect(),
        };

        Ok(tonic::Response::new(response))
    }

    async fn create_dataset_entry(
        &self,
        request: tonic::Request<re_protos::catalog::v1alpha1::CreateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::CreateDatasetEntryResponse>,
        tonic::Status,
    > {
        let dataset_name: String = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        let entry_id = store.create_dataset(&dataset_name).map_err(|err| {
            tonic::Status::internal(format!("Failed to create dataset entry: {err:#}"))
        })?;

        let dataset_entry = store
            .dataset(entry_id)
            .expect("was just successfully created")
            .as_dataset_entry();

        Ok(tonic::Response::new(
            CreateDatasetEntryResponse {
                dataset: dataset_entry,
            }
            .into(),
        ))
    }

    async fn read_dataset_entry(
        &self,
        request: tonic::Request<re_protos::catalog::v1alpha1::ReadDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::ReadDatasetEntryResponse>,
        tonic::Status,
    > {
        let entry_id = request.into_inner().try_into()?;

        let store = self.store.read().await;
        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
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
        _request: tonic::Request<re_protos::catalog::v1alpha1::UpdateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::UpdateDatasetEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "update_dataset_entry not implemented",
        ))
    }

    async fn read_table_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::ReadTableEntryRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::catalog::v1alpha1::ReadTableEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "read_table_entry not implemented",
        ))
    }

    async fn delete_entry(
        &self,
        request: tonic::Request<re_protos::catalog::v1alpha1::DeleteEntryRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::DeleteEntryResponse>, tonic::Status>
    {
        let entry_id = request.into_inner().try_into()?;

        self.store.write().await.delete_dataset(entry_id)?;

        Ok(tonic::Response::new(DeleteEntryResponse {}))
    }

    // --- Manifest Registry ---

    /* Write data */

    async fn register_with_dataset(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::RegisterWithDatasetRequest>,
    ) -> Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::RegisterWithDatasetResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "register_with_dataset not implemented",
        ))
    }

    async fn write_chunks(
        &self,
        request: tonic::Request<
            tonic::Streaming<re_protos::manifest_registry::v1alpha1::WriteChunksRequest>,
        >,
    ) -> Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::WriteChunksResponse>,
        tonic::Status,
    > {
        // TODO(ab): add a helper somewhere for this conversion
        let dataset_id = request
            .metadata()
            .get("x-rerun-dataset-id")
            .cloned()
            .ok_or_else(|| {
                tonic::Status::not_found("'x-rerun-dataset-id' not provided in the headers")
            })?;

        let dataset_id: re_tuid::Tuid = dataset_id
            .to_str()
            .map_err(|_err| {
                tonic::Status::unknown("could not convert dataset id header to string")
            })?
            .parse()
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("could not parse dataset id: {err:#}"))
            })?;

        let entry_id: EntryId = EntryId::from(dataset_id);

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
                        dataset_id.to_string(),
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
            re_protos::manifest_registry::v1alpha1::WriteChunksResponse {},
        ))
    }

    /* Query schemas */

    async fn get_partition_table_schema(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::GetPartitionTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::GetPartitionTableSchemaResponse>,
        tonic::Status,
    > {
        let entry_id = request.into_inner().try_into()?;

        let store = self.store.read().await;

        // check that the dataset exists before returning
        store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        Ok(tonic::Response::new(GetPartitionTableSchemaResponse {
            schema: Some(
                (&ScanPartitionTableResponse::schema())
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!(
                            "Unable to serialize Arrow schema: {err:#}"
                        ))
                    })?,
            ),
        }))
    }

    type ScanPartitionTableStream = ScanPartitionTableResponseStream;

    async fn scan_partition_table(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::ScanPartitionTableRequest>,
    ) -> Result<tonic::Response<Self::ScanPartitionTableStream>, tonic::Status> {
        let request: ScanPartitionTableRequest = request.into_inner().try_into()?;
        if request.scan_parameters.is_some() {
            return Err(tonic::Status::unimplemented(
                "scan_partition_table: scan_parameters not implemented",
            ));
        }
        let entry_id = request.dataset_id;

        let store = self.store.read().await;
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
        request: tonic::Request<re_protos::frontend::v1alpha1::GetDatasetSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::GetDatasetSchemaResponse>,
        tonic::Status,
    > {
        let entry_id = request.into_inner().try_into()?;

        let store = self.store.read().await;
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
        _request: tonic::Request<re_protos::frontend::v1alpha1::CreateIndexRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::CreateIndexResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("create_index not implemented"))
    }

    async fn re_index(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::ReIndexRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::ReIndexResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("re_index not implemented"))
    }

    /* Queries */

    type SearchDatasetStream = SearchDatasetResponseStream;

    async fn search_dataset(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::SearchDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::SearchDatasetStream>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "search_dataset not implemented",
        ))
    }

    type QueryDatasetStream = QueryDatasetResponseStream;

    async fn query_dataset(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::QueryDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::QueryDatasetStream>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "query_dataset not implemented",
        ))
    }

    type GetChunksStream = GetChunksResponseStream;

    async fn get_chunks(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::GetChunksRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetChunksStream>, tonic::Status> {
        let GetChunksRequest {
            dataset_id,
            mut partition_ids,
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

        let store = self.store.read().await;
        let dataset = store.dataset(dataset_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {dataset_id} not found"))
        })?;

        if partition_ids.is_empty() {
            partition_ids = dataset.partition_ids().collect();
        }

        let storage_engines = partition_ids
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
            .collect::<Result<Vec<_>, _>>()?;

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
        _request: tonic::Request<re_protos::frontend::v1alpha1::GetTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::frontend::v1alpha1::GetTableSchemaResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "get_table_schema not implemented",
        ))
    }

    type ScanTableStream = ScanTableResponseStream;

    async fn scan_table(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::ScanTableRequest>,
    ) -> std::result::Result<tonic::Response<Self::ScanTableStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("scan_table not implemented"))
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
        Err(tonic::Status::unimplemented(
            "query_tasks_on_completion not implemented",
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
        _request: tonic::Request<re_protos::catalog::v1alpha1::UpdateEntryRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::UpdateEntryResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented("update_entry not implemented"))
    }

    async fn do_maintenance(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::DoMaintenanceRequest>,
    ) -> Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::DoMaintenanceResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "do_maintenance not implemented",
        ))
    }
}
