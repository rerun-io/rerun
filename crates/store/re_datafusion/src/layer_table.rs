use std::sync::Arc;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use async_trait::async_trait;
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tracing::instrument;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::{
    cloud::v1alpha1::{ScanLayerTableRequest, ScanLayerTableResponse},
    headers::RerunHeadersInjectorExt as _,
};
use re_redap_client::ConnectionClient;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

//TODO(ab): deduplicate from PartitionTableProvider
#[derive(Clone)]
pub struct LayerTableProvider {
    client: ConnectionClient,
    dataset_id: EntryId,
}

impl std::fmt::Debug for LayerTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerTableProvider")
            .field("dataset_id", &self.dataset_id)
            .finish()
    }
}

impl LayerTableProvider {
    pub fn new(client: ConnectionClient, dataset_id: EntryId) -> Self {
        Self { client, dataset_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> DataFusionResult<Arc<dyn TableProvider>> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for LayerTableProvider {
    type GrpcStreamData = ScanLayerTableResponse;

    #[instrument(skip(self), err)]
    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let mut client = self.client.clone();

        let dataset_id = self.dataset_id;

        Ok(Arc::new(
            make_future_send(async move {
                client
                    .get_layer_table_schema(dataset_id)
                    .await
                    .map_err(|err| {
                        DataFusionError::External(
                            format!("Couldn't get partition table schema: {err}").into(),
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
        let request = tonic::Request::new(ScanLayerTableRequest {
            columns: vec![], // all of them
        })
        .with_entry_id(self.dataset_id)
        .map_err(|err| DataFusionError::External(Box::new(err)))?;

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().scan_layer_table(request).await) })
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
                "DataFrame missing from PartitionList response".to_owned(),
            ))?
            .decode()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }
}
