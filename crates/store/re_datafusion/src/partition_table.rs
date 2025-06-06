use std::sync::Arc;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use async_trait::async_trait;
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};

use re_grpc_client::ConnectionClient;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::frontend::v1alpha1::GetPartitionTableSchemaRequest;
use re_protos::{
    frontend::v1alpha1::ScanPartitionTableRequest,
    manifest_registry::v1alpha1::ScanPartitionTableResponse,
};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Debug, Clone)]
pub struct PartitionTableProvider {
    //TODO(ab): this should use a `ConnectionRegistryHandle` instead
    client: ConnectionClient,
    dataset_id: EntryId,
}

impl PartitionTableProvider {
    pub fn new(client: ConnectionClient, dataset_id: EntryId) -> Self {
        Self { client, dataset_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> DataFusionResult<Arc<dyn TableProvider>> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for PartitionTableProvider {
    type GrpcStreamData = ScanPartitionTableResponse;

    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let request = GetPartitionTableSchemaRequest {
            dataset_id: Some(self.dataset_id.into()),
        };

        let mut client = self.client.clone();

        Ok(Arc::new(
            make_future_send(async move {
                Ok(client.inner().get_partition_table_schema(request).await)
            })
            .await?
            .map_err(|err| DataFusionError::External(Box::new(err)))?
            .into_inner()
            .schema
            .ok_or(DataFusionError::External(
                "Schema missing from GetPartitionTableSchema response".into(),
            ))?
            .try_into()?,
        ))
    }

    async fn send_streaming_request(
        &mut self,
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>> {
        let request = ScanPartitionTableRequest {
            dataset_id: Some(self.dataset_id.into()),
            scan_parameters: None,
        };

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().scan_partition_table(request).await) })
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
