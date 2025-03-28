use std::sync::Arc;

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    common::v1alpha1::ext::EntryId,
    frontend::v1alpha1::{
        frontend_service_client::FrontendServiceClient, GetTableSchemaRequest, ScanTableRequest,
        ScanTableResponse,
    },
};
use tonic::transport::Channel;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct TableEntryProvider {
    client: FrontendServiceClient<Channel>,
    table_id: EntryId,
}

impl TableEntryProvider {
    pub fn new(client: FrontendServiceClient<Channel>, table_id: EntryId) -> Self {
        Self { client, table_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Arc<dyn TableProvider> {
        GrpcStreamProvider::prepare(self).await
    }
}

#[async_trait]
impl GrpcStreamToTable for TableEntryProvider {
    type GrpcStreamData = ScanTableResponse;

    async fn create_schema(&mut self) -> SchemaRef {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id.into()),
        };

        Arc::new(
            self.client
                .get_table_schema(request)
                .await
                .unwrap()
                .into_inner()
                .schema
                .unwrap()
                .try_into()
                .unwrap(),
        )
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let request = ScanTableRequest {
            table_id: Some(self.table_id.into()),
        };

        self.client.scan_table(request).await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        response
            .dataframe_part
            .ok_or(DataFusionError::Execution(
                "DataFrame missing from PartitionList response".to_owned(),
            ))?
            .decode()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }
}
