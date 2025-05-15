use std::sync::{Arc, RwLock, RwLockReadGuard};

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use tonic::Status;

use re_log_encoding::codec::wire::encoder::Encode as _;
use re_protos::catalog::v1alpha1::ext::ReadDatasetEntryResponse;
use re_protos::frontend::v1alpha1::ext::GetChunksRequest;
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
    settings: FrontendHandlerSettings,

    store: std::sync::RwLock<InMemoryStore>,
}

impl FrontendHandler {
    pub fn new(settings: FrontendHandlerSettings, store: InMemoryStore) -> Self {
        Self {
            settings,
            store: RwLock::new(store),
        }
    }

    fn read_store<'a>(&'a self) -> Result<RwLockReadGuard<'a, InMemoryStore>, tonic::Status> {
        self.store
            .read()
            .map_err(|_| Status::resource_exhausted("failed to acquire lock"))
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
decl_stream!(WriteChunksResponseStream<manifest:WriteChunksResponse>);
decl_stream!(ScanTableResponseStream<frontend:ScanTableResponse>);
decl_stream!(QueryTasksOnCompletionResponseStream<tasks:QueryTasksOnCompletionResponse>);

#[tonic::async_trait]
impl FrontendService for FrontendHandler {
    // --- Catalog ---

    async fn find_entries(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::FindEntriesRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::FindEntriesResponse>, tonic::Status>
    {
        let store = self.read_store()?;
        let response = re_protos::catalog::v1alpha1::FindEntriesResponse {
            entries: store
                .iter_datasets()
                .map(Dataset::as_entry_details)
                .map(Into::into)
                .collect(),
        };

        Ok(tonic::Response::new(response))
    }

    async fn create_dataset_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::CreateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::CreateDatasetEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "create_dataset_entry not implemented",
        ))
    }

    async fn read_dataset_entry(
        &self,
        request: tonic::Request<re_protos::catalog::v1alpha1::ReadDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::ReadDatasetEntryResponse>,
        tonic::Status,
    > {
        //TODO: add ext helper
        let entry_id = request
            .into_inner()
            .id
            .ok_or_else(|| {
                tonic::Status::invalid_argument("Missing entry ID in ReadDatasetEntryRequest")
            })?
            .try_into()?;

        let store = self.read_store()?;
        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        Ok(tonic::Response::new(
            ReadDatasetEntryResponse {
                dataset_entry: dataset.as_dataset_entry().into(),
            }
            .into(),
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
        _request: tonic::Request<re_protos::catalog::v1alpha1::DeleteEntryRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::DeleteEntryResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented("delete_entry not implemented"))
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
        _request: tonic::Request<
            tonic::Streaming<re_protos::manifest_registry::v1alpha1::WriteChunksRequest>,
        >,
    ) -> Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::WriteChunksResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("write_chunks not implemented"))
    }

    /* Query schemas */

    async fn get_partition_table_schema(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::GetPartitionTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::GetPartitionTableSchemaResponse>,
        tonic::Status,
    > {
        // ensure we know about the requested entry
        let entry_id = request
            .into_inner()
            .dataset_id
            .ok_or_else(|| {
                tonic::Status::invalid_argument(
                    "Missing dataset ID in GetPartitionTableSchemaRequest",
                )
            })?
            .try_into()?;

        let store = self.read_store()?;
        let _ = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        Ok(tonic::Response::new(GetPartitionTableSchemaResponse {
            schema: Some(
                (&ScanPartitionTableResponse::schema())
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!("Unable to serialize Arrow schema: {err}"))
                    })?,
            ),
        }))
    }

    type ScanPartitionTableStream = ScanPartitionTableResponseStream;

    async fn scan_partition_table(
        &self,
        request: tonic::Request<re_protos::frontend::v1alpha1::ScanPartitionTableRequest>,
    ) -> Result<tonic::Response<Self::ScanPartitionTableStream>, tonic::Status> {
        let request = request.into_inner();
        if request.scan_parameters.is_some() {
            return Err(tonic::Status::unimplemented(
                "scan_partition_table: scan_parameters not implemented",
            ));
        }
        let entry_id = request
            .dataset_id
            .ok_or_else(|| {
                tonic::Status::invalid_argument("Missing dataset ID in ScanPartitionTableRequest")
            })?
            .try_into()?;

        let store = self.read_store()?;
        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        let record_batch = dataset.partition_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read partition table: {err}"))
        })?;

        let stream = futures::stream::once(async move {
            record_batch
                .encode()
                .map(|data| ScanPartitionTableResponse { data: Some(data) })
                .map_err(|err| tonic::Status::internal(format!("failed encoding metadata: {err}")))
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
        let entry_id = request
            .into_inner()
            .dataset_id
            .ok_or_else(|| {
                tonic::Status::invalid_argument("Missing dataset ID in GetDatasetSchemaRequest")
            })?
            .try_into()?;

        let store = self.read_store()?;
        let dataset = store.dataset(entry_id).ok_or_else(|| {
            tonic::Status::not_found(format!("Entry with ID {entry_id} not found"))
        })?;

        let schema = dataset.schema().map_err(|err| {
            tonic::Status::internal(format!("Unable to read dataset schema: {err}"))
        })?;

        Ok(tonic::Response::new(GetDatasetSchemaResponse {
            schema: Some((&schema).try_into().map_err(|err| {
                tonic::Status::internal(format!("Unable to serialize Arrow schema: {err}"))
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

            // We don't support querying for specific chunks, so you always get everything
            entity_paths: _,
            query: _,
        } = GetChunksRequest::try_from(request.into_inner())?;

        if !chunk_ids.is_empty() {
            return Err(tonic::Status::unimplemented(
                "get_chunks: querying specific chunk ids is not implemented",
            ));
        }

        let store = self.read_store()?;
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
                    .partition(partition_id.clone())
                    .ok_or_else(|| {
                        tonic::Status::not_found(format!(
                            "Partition with ID {partition_id} not found"
                        ))
                    })
                    .map(|partition| {
                        #[expect(unsafe_code)]
                        unsafe { partition.storage_engine_raw() }.clone()
                    })
                    .map(|storage_engine| (partition_id, storage_engine))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let stream = futures::stream::iter(storage_engines.into_iter().flat_map(
            |(partition_id, storage_engine)| {
                storage_engine
                    .read()
                    .store()
                    .iter_chunks()
                    .map(|chunk| {
                        let record_batch: RecordBatch = chunk
                            .to_chunk_batch()
                            .map_err(|err| {
                                tonic::Status::internal(format!(
                                    "Unable to convert chunk to RecordBatch: {err}"
                                ))
                            })?
                            .into();

                        let schema = (*record_batch.schema()).clone();
                        let mut metadata = schema.metadata().clone();
                        metadata.insert("rerun.partition_id".to_owned(), partition_id.id.clone());
                        let new_schema = Schema::new_with_metadata(schema.fields, metadata);
                        let record_batch = record_batch
                            .with_schema(Arc::new(new_schema))
                            .expect("can't fail, was a sane record batch to begin with");

                        record_batch
                            .encode()
                            .map(|data| GetChunksResponse { chunk: Some(data) })
                            .map_err(|err| {
                                tonic::Status::internal(format!("failed encoding metadata: {err}"))
                            })
                    })
                    .collect::<Vec<_>>()
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::GetChunksStream
        ))
    }

    // --- Table APIs ---

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
}
