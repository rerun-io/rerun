use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::TableProvider;
use datafusion::error::{DataFusionError, Result as DataFusionResult};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{ScanDatasetManifestRequest, ScanDatasetManifestResponse};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::ConnectionClient;
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
            .finish()
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
    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let mut client = self.client.clone();

        let dataset_id = self.dataset_id;

        Ok(Arc::new(
            make_future_send(async move {
                client
                    .get_dataset_manifest_schema(dataset_id)
                    .await
                    .map_err(|err| {
                        DataFusionError::External(
                            format!("Couldn't get dataset manifest schema: {err}").into(),
                        )
                    })
            })
            .await?,
        ))
    }

    // TODO(ab): what `GrpcStreamToTable` attempts to simplify should probably be handled by
    // `ConnectionClient`
    #[instrument(skip(self), err)]
    async fn send_streaming_request(
        &mut self,
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>> {
        let request = tonic::Request::new(ScanDatasetManifestRequest {
            columns: vec![], // all of them
        })
        .with_entry_id(self.dataset_id)
        .map_err(|err| DataFusionError::External(Box::new(err)))?;

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().scan_dataset_manifest(request).await) })
            .await?
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        response
            .data
            .ok_or(DataFusionError::Execution(
                "DataFrame missing from DatasetManifest response".to_owned(),
            ))?
            .try_into()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }
}
