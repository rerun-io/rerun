use std::sync::Arc;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use async_trait::async_trait;
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tonic::transport::Channel;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::frontend::v1alpha1::GetPartitionTableSchemaRequest;
use re_protos::{
    frontend::v1alpha1::{
        frontend_service_client::FrontendServiceClient, ScanPartitionTableRequest,
    },
    manifest_registry::v1alpha1::ScanPartitionTableResponse,
};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct PartitionTableProvider {
    client: FrontendServiceClient<Channel>,
    dataset_id: EntryId,
}

impl PartitionTableProvider {
    pub fn new(client: FrontendServiceClient<Channel>, dataset_id: EntryId) -> Self {
        Self { client, dataset_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for PartitionTableProvider {
    type GrpcStreamData = ScanPartitionTableResponse;

    async fn fetch_schema(&mut self) -> Result<SchemaRef, DataFusionError> {
        let request = GetPartitionTableSchemaRequest {
            dataset_id: Some(self.dataset_id.into()),
        };

        Ok(Arc::new(
            self.client
                .get_partition_table_schema(request)
                .await
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
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let request = ScanPartitionTableRequest {
            dataset_id: Some(self.dataset_id.into()),
            scan_parameters: None,
        };

        self.client.scan_partition_table(request).await
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
