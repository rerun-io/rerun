use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use arrow::{
    array::RecordBatch,
    datatypes::{DataType, Field, Schema, SchemaRef},
};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::external::{re_tuid::Tuid, re_types_core::Loggable as _};
use re_protos::{
    frontend::v1alpha1::{
        frontend_service_client::FrontendServiceClient, ScanPartitionTableRequest,
    },
    manifest_registry::v1alpha1::ScanPartitionTableResponse,
};
use tonic::transport::Channel;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct PartitionTableProvider {
    client: FrontendServiceClient<Channel>,
    tuid: Tuid,
}

impl PartitionTableProvider {
    pub fn new(client: FrontendServiceClient<Channel>, tuid: Tuid) -> Self {
        Self { client, tuid }
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
        // TODO(jleibs): actually fetch from front-end
        Ok(Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("id", Tuid::arrow_datatype(), true),
                Field::new("name", DataType::Utf8, true),
                Field::new("entry_type", DataType::Int32, true),
                Field::new("created_at", DataType::Int64, true),
                Field::new("updated_at", DataType::Int64, true),
            ],
            HashMap::default(),
        )))
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let request = ScanPartitionTableRequest {
            dataset_id: Some(self.tuid.into()),
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
