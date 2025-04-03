use std::sync::Arc;

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tonic::transport::Channel;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::frontend::v1alpha1::{
    frontend_service_client::FrontendServiceClient, GetTableSchemaRequest, ScanTableRequest,
    ScanTableResponse,
};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct TableEntryTableProvider {
    client: FrontendServiceClient<Channel>,
    table_id: EntryId,
}

impl TableEntryTableProvider {
    pub fn new(client: FrontendServiceClient<Channel>, table_id: EntryId) -> Self {
        Self { client, table_id }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for TableEntryTableProvider {
    type GrpcStreamData = ScanTableResponse;

    async fn fetch_schema(&mut self) -> Result<SchemaRef, DataFusionError> {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id.into()),
        };

        Ok(Arc::new(
            self.client
                .get_table_schema(request)
                .await
                .map_err(|err| DataFusionError::External(Box::new(err)))?
                .into_inner()
                .schema
                .ok_or(DataFusionError::External(
                    "Schema missing from GetTableSchema response".into(),
                ))?
                .try_into()?,
        ))
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
