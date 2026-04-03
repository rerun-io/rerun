use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::TableProvider;
use datafusion::error::DataFusionError;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{SearchDatasetRequest, SearchDatasetResponse};
use re_protos::common::v1alpha1::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ApiError, ApiResult, ConnectionClient};
use tokio_stream::StreamExt as _;
use tracing::instrument;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Clone)]
pub struct SearchResultsTableProvider {
    client: ConnectionClient,
    dataset_id: EntryId,
    request: SearchDatasetRequest,
}

impl std::fmt::Debug for SearchResultsTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchResultsTableProvider")
            .field("request", &self.request)
            .finish()
    }
}

impl SearchResultsTableProvider {
    pub fn new(
        client: ConnectionClient,
        dataset_id: EntryId,
        request: SearchDatasetRequest,
    ) -> Result<Self, DataFusionError> {
        if request.scan_parameters.is_some() {
            return Err(DataFusionError::External(
                "Scan parameters are not supported for SearchResultsTableProvider".into(),
            ));
        }

        Ok(Self {
            client,
            dataset_id,
            request,
        })
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for SearchResultsTableProvider {
    type GrpcStreamData = SearchDatasetResponse;

    #[instrument(skip(self), err)]
    async fn fetch_schema(&mut self) -> ApiResult<SchemaRef> {
        let mut request = self.request.clone();
        request.scan_parameters = Some(ScanParameters {
            limit_len: Some(0),
            ..Default::default()
        });

        let mut client = self.client.clone();
        let dataset_id = self.dataset_id;

        let mut stream = make_future_send(async move {
            let response = client
                .inner()
                .search_dataset(
                    tonic::Request::new(request)
                        .with_entry_id(dataset_id)
                        .map_err(|err| {
                            ApiError::tonic(err, "failed building /SearchDataset schema request")
                        })?,
                )
                .await
                .map_err(|err| ApiError::tonic(err, "/SearchDataset schema request failed"))?;
            Ok(re_redap_client::ApiResponseStream::from_tonic_response(
                response,
                "/SearchDataset",
            ))
        })
        .await?;

        let trace_id = stream.trace_id();

        let rb: RecordBatch = stream
            .next()
            .await
            .ok_or_else(|| ApiError::deserialization(None, "Empty stream from search results"))??
            .data
            .ok_or_else(|| ApiError::deserialization(None, "Empty data from search results"))?
            .try_into()
            .map_err(|err: re_protos::TypeConversionError| {
                ApiError::deserialization_with_source(
                    trace_id,
                    err,
                    "failed decoding /SearchDataset schema response",
                )
            })?;

        Ok(rb.schema())
    }

    #[instrument(skip(self), err)]
    async fn send_streaming_request(
        &mut self,
    ) -> ApiResult<re_redap_client::ApiResponseStream<Self::GrpcStreamData>> {
        let request = tonic::Request::new(self.request.clone())
            .with_entry_id(self.dataset_id)
            .map_err(|err| ApiError::tonic(err, "failed building /SearchDataset request"))?;

        let mut client = self.client.clone();

        let response = make_future_send(async move {
            client
                .inner()
                .search_dataset(request)
                .await
                .map_err(|err| ApiError::tonic(err, "/SearchDataset failed"))
        })
        .await?;

        Ok(re_redap_client::ApiResponseStream::from_tonic_response(
            response,
            "/SearchDataset",
        ))
    }

    fn process_response(&mut self, response: Self::GrpcStreamData) -> ApiResult<RecordBatch> {
        response
            .data
            .ok_or_else(|| {
                ApiError::deserialization(
                    None,
                    "DataFrame missing from SearchDataResponse response",
                )
            })?
            .try_into()
            .map_err(|err: re_protos::TypeConversionError| {
                ApiError::deserialization_with_source(
                    None,
                    err,
                    "failed decoding /SearchDataset response",
                )
            })
    }
}
