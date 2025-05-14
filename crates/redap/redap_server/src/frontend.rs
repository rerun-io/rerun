use re_protos::{
    frontend::v1alpha1::frontend_service_server::FrontendService,
    redap_tasks::v1alpha1::{
        FetchTaskOutputRequest, FetchTaskOutputResponse, QueryTasksOnCompletionRequest,
        QueryTasksRequest, QueryTasksResponse,
    },
};

// ---

#[derive(Debug, Default)]
pub struct FrontendHandlerSettings {}

pub struct FrontendHandlerBuilder {
    settings: FrontendHandlerSettings,
}

impl FrontendHandlerBuilder {
    pub fn new() -> Self {
        Self {
            settings: FrontendHandlerSettings::default(),
        }
    }

    pub fn build(self) -> FrontendHandler {
        FrontendHandler::new(self.settings)
    }
}

// ---

pub struct FrontendHandler {
    settings: FrontendHandlerSettings,
}

impl FrontendHandler {
    pub fn new(settings: FrontendHandlerSettings) -> Self {
        Self { settings }
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
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn create_dataset_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::CreateDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::CreateDatasetEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn read_dataset_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::ReadDatasetEntryRequest>,
    ) -> Result<
        tonic::Response<re_protos::catalog::v1alpha1::ReadDatasetEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn read_table_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::ReadTableEntryRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::catalog::v1alpha1::ReadTableEntryResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn delete_entry(
        &self,
        _request: tonic::Request<re_protos::catalog::v1alpha1::DeleteEntryRequest>,
    ) -> Result<tonic::Response<re_protos::catalog::v1alpha1::DeleteEntryResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented("come back later"))
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
        Err(tonic::Status::unimplemented("come back later"))
    }

    type WriteChunksStream = WriteChunksResponseStream;

    async fn write_chunks(
        &self,
        _request: tonic::Request<
            tonic::Streaming<re_protos::frontend::v1alpha1::WriteChunksRequest>,
        >,
    ) -> Result<tonic::Response<Self::WriteChunksStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    /* Query schemas */

    async fn get_partition_table_schema(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::GetPartitionTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::GetPartitionTableSchemaResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    type ScanPartitionTableStream = ScanPartitionTableResponseStream;

    async fn scan_partition_table(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::ScanPartitionTableRequest>,
    ) -> Result<tonic::Response<Self::ScanPartitionTableStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn get_dataset_schema(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::GetDatasetSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::GetDatasetSchemaResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    /* Indexing */

    async fn create_index(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::CreateIndexRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::CreateIndexResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn re_index(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::ReIndexRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::manifest_registry::v1alpha1::ReIndexResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    /* Queries */

    type SearchDatasetStream = SearchDatasetResponseStream;

    async fn search_dataset(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::SearchDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::SearchDatasetStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    type QueryDatasetStream = QueryDatasetResponseStream;

    async fn query_dataset(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::QueryDatasetRequest>,
    ) -> std::result::Result<tonic::Response<Self::QueryDatasetStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    type GetChunksStream = GetChunksResponseStream;

    async fn get_chunks(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::GetChunksRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetChunksStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    // --- Table APIs ---

    async fn get_table_schema(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::GetTableSchemaRequest>,
    ) -> std::result::Result<
        tonic::Response<re_protos::frontend::v1alpha1::GetTableSchemaResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented("come back later"))
    }

    type ScanTableStream = ScanTableResponseStream;

    async fn scan_table(
        &self,
        _request: tonic::Request<re_protos::frontend::v1alpha1::ScanTableRequest>,
    ) -> std::result::Result<tonic::Response<Self::ScanTableStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    // --- Tasks service ---

    async fn query_tasks(
        &self,
        _request: tonic::Request<QueryTasksRequest>,
    ) -> Result<tonic::Response<QueryTasksResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    type QueryTasksOnCompletionStream = QueryTasksOnCompletionResponseStream;

    async fn query_tasks_on_completion(
        &self,
        _request: tonic::Request<QueryTasksOnCompletionRequest>,
    ) -> Result<tonic::Response<Self::QueryTasksOnCompletionStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }

    async fn fetch_task_output(
        &self,
        _request: tonic::Request<FetchTaskOutputRequest>,
    ) -> Result<tonic::Response<FetchTaskOutputResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("come back later"))
    }
}
