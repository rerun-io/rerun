use futures::Stream;
use re_datafusion::DataframeClientAPI;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, FetchChunksResponse, FindChunksRequest, FindChunksResponse,
    GetDatasetSchemaRequest, GetDatasetSchemaResponse, QueryDatasetRequest, QueryDatasetResponse,
};
use std::collections::VecDeque;
use std::fmt::Formatter;
use std::sync::Arc;
use tonic::codec::DecodeBuf;
use tonic::{Request, Response, Status};

/// One request this [`TestClient`] recorded, in whichever protocol the provider under test used.
#[derive(Clone, Debug)]
pub(crate) enum RecordedRequest {
    QueryDataset(re_protos::cloud::v1alpha1::ext::QueryDatasetRequest),
    FindChunks(re_protos::cloud::v1alpha1::ext::FindChunksRequest),
}

pub(crate) struct TestClient<T: RerunCloudService> {
    pub(crate) origin: re_uri::Origin,

    pub(crate) service: Arc<T>,

    /// Every `query_dataset` or `find_chunks` request seen by this client, in order,
    /// converted with the same `TryFrom` the servers validate against. Lets a test assert
    /// what request the provider emitted (e.g. that per-segment-value pushdown fired) without
    /// caring which protocol carried it. Shared across clones so a clone handed to a provider
    /// records into the same log the test reads back.
    pub(crate) recorded_requests: Arc<parking_lot::Mutex<Vec<RecordedRequest>>>,
}

impl<T: RerunCloudService> TestClient<T> {
    pub(crate) fn new(service: Arc<T>) -> Self {
        Self {
            origin: re_uri::Origin::test(),
            service,
            recorded_requests: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

// Derive macros complain about unsatisfied bounds, so implement manually
impl<T: RerunCloudService> Clone for TestClient<T> {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            service: Arc::clone(&self.service),
            recorded_requests: Arc::clone(&self.recorded_requests),
        }
    }
}

impl<T: RerunCloudService> std::fmt::Debug for TestClient<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestClient").finish()
    }
}

/// Adapter to convert a stream into tonic Streaming format
struct StreamAdapter<T> {
    results: VecDeque<tonic::Result<T>>,
}

impl<T: std::fmt::Debug> tonic::codec::Decoder for StreamAdapter<T> {
    type Item = T;
    type Error = Status;

    fn decode(&mut self, _src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        self.results.pop_front().transpose()
    }
}

/// Helper to convert a stream to `tonic::codec::Streaming`. Since this is only
/// for integration tests, we simplify the problem by collecting everything
/// into memory and then "decoding" one at a time, but really we just pop
/// from the front of the queue.
async fn stream_to_streaming<T, S>(stream: S) -> tonic::codec::Streaming<T>
where
    T: Send + 'static + std::fmt::Debug,
    S: Stream<Item = tonic::Result<T>> + Send + 'static,
{
    use futures::StreamExt as _;
    let results: VecDeque<tonic::Result<T>> = stream.collect().await;

    let body_len = 5 * results.len(); // compression_flag (1) + length (4)
    let adapter = StreamAdapter { results };

    let body_bytes = vec![0u8; body_len];
    Request::new(tonic::codec::Streaming::new_request(
        adapter,
        String::from_utf8(body_bytes).unwrap(),
        None,
        Some(1),
    ))
    .into_inner()
}

#[async_trait::async_trait]
impl<T: RerunCloudService> DataframeClientAPI for TestClient<T> {
    fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    async fn get_dataset_schema(
        &mut self,
        request: Request<GetDatasetSchemaRequest>,
    ) -> tonic::Result<Response<GetDatasetSchemaResponse>> {
        self.service.get_dataset_schema(request).await
    }

    async fn query_dataset(
        &mut self,
        request: Request<QueryDatasetRequest>,
    ) -> tonic::Result<Response<tonic::codec::Streaming<QueryDatasetResponse>>> {
        let recorded = re_protos::cloud::v1alpha1::ext::QueryDatasetRequest::try_from(
            request.get_ref().clone(),
        )?;
        self.recorded_requests
            .lock()
            .push(RecordedRequest::QueryDataset(recorded));

        let response = self.service.query_dataset(request).await?;
        let (metadata, stream, _extensions) = response.into_parts();

        let streaming = stream_to_streaming(stream).await;

        let mut new_response = Response::new(streaming);
        *new_response.metadata_mut() = metadata;
        Ok(new_response)
    }

    async fn find_chunks(
        &mut self,
        request: Request<FindChunksRequest>,
    ) -> tonic::Result<Response<tonic::codec::Streaming<FindChunksResponse>>> {
        let recorded = re_protos::cloud::v1alpha1::ext::FindChunksRequest::try_from(
            request.get_ref().clone(),
        )?;
        self.recorded_requests
            .lock()
            .push(RecordedRequest::FindChunks(recorded));

        let response = self.service.find_chunks(request).await?;
        let (metadata, stream, _extensions) = response.into_parts();

        let streaming = stream_to_streaming(stream).await;

        let mut new_response = Response::new(streaming);
        *new_response.metadata_mut() = metadata;
        Ok(new_response)
    }

    async fn fetch_chunks(
        &mut self,
        request: Request<FetchChunksRequest>,
    ) -> tonic::Result<Response<tonic::codec::Streaming<FetchChunksResponse>>> {
        let response = self.service.fetch_chunks(request).await?;
        let (metadata, stream, _extensions) = response.into_parts();

        let streaming = stream_to_streaming(stream).await;

        let mut new_response = Response::new(streaming);
        *new_response.metadata_mut() = metadata;
        Ok(new_response)
    }
}

pub async fn create_test_client<T>(service: T) -> TestClient<T>
where
    T: RerunCloudService,
{
    TestClient::new(Arc::new(service))
}
