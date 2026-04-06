use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::TableProvider;
use datafusion::error::Result as DataFusionResult;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{ScanDatasetManifestRequest, ScanDatasetManifestResponse};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ApiError, ApiResult, ConnectionClient};
use tracing::instrument;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

//TODO(ab): deduplicate from SegmentTableProvider
#[derive(Clone)]
pub struct DatasetManifestProvider {
    client: ConnectionClient,
    dataset_id: EntryId,
}

impl std::fmt::Debug for DatasetManifestProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatasetManifestProvider")
            .field("dataset_id", &self.dataset_id)
            .finish_non_exhaustive()
    }
}

impl DatasetManifestProvider {
    pub fn new(client: ConnectionClient, dataset_id: EntryId) -> Self {
        Self { client, dataset_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> DataFusionResult<Arc<dyn TableProvider>> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for DatasetManifestProvider {
    type GrpcStreamData = ScanDatasetManifestResponse;

    #[instrument(skip(self), err)]
    async fn fetch_schema(&mut self) -> ApiResult<SchemaRef> {
        let mut client = self.client.clone();
        let dataset_id = self.dataset_id;

        Ok(Arc::new(
            make_future_send(async move { client.get_dataset_manifest_schema(dataset_id).await })
                .await?,
        ))
    }

    // TODO(ab): what `GrpcStreamToTable` attempts to simplify should probably be handled by
    // `ConnectionClient`
    #[instrument(skip(self), err)]
    async fn send_streaming_request(
        &mut self,
    ) -> ApiResult<re_redap_client::ApiResponseStream<Self::GrpcStreamData>> {
        let request = tonic::Request::new(ScanDatasetManifestRequest {
            columns: vec![], // all of them
        })
        .with_entry_id(self.dataset_id)
        .map_err(|err| ApiError::tonic(err, "failed building /ScanDatasetManifest request"))?;

        let mut client = self.client.clone();

        let response = make_future_send(async move {
            client
                .inner()
                .scan_dataset_manifest(request)
                .await
                .map_err(|err| ApiError::tonic(err, "/ScanDatasetManifest failed"))
        })
        .await?;

        Ok(re_redap_client::ApiResponseStream::from_tonic_response(
            response,
            "/ScanDatasetManifest",
        ))
    }

    fn process_response(&mut self, response: Self::GrpcStreamData) -> ApiResult<RecordBatch> {
        response
            .data
            .ok_or_else(|| {
                ApiError::deserialization(None, "DataFrame missing from DatasetManifest response")
            })?
            .try_into()
            .map_err(|err: re_protos::TypeConversionError| {
                ApiError::deserialization_with_source(
                    None,
                    err,
                    "failed decoding /ScanDatasetManifest response",
                )
            })
    }
}
