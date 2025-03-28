use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use arrow::{
    array::RecordBatch,
    datatypes::{DataType, Field, Fields, Schema, SchemaRef},
};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::external::re_tuid::Tuid;
use re_protos::{
    common::v1alpha1::DatasetHandle,
    manifest_registry::v1alpha1::{
        manifest_registry_service_client::ManifestRegistryServiceClient, ListPartitionsRequest,
        ListPartitionsResponse,
    },
};
use tonic::transport::Channel;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct PartitionListProvider {
    client: ManifestRegistryServiceClient<Channel>,
    tuid: Tuid,
    url: String,
}

impl PartitionListProvider {
    pub fn new(conn: Channel, tuid: Tuid, url: impl Into<String>) -> Self {
        Self {
            client: ManifestRegistryServiceClient::new(conn),
            tuid,
            url: url.into(),
        }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Arc<dyn TableProvider> {
        GrpcStreamProvider::prepare(self).await
    }
}

#[async_trait]
impl GrpcStreamToTable for PartitionListProvider {
    type GrpcStreamData = ListPartitionsResponse;

    async fn create_schema(&mut self) -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new(
                    "id",
                    DataType::Struct(Fields::from([
                        Arc::new(Field::new("time_ns", DataType::UInt64, true)),
                        Arc::new(Field::new("inc", DataType::UInt64, true)),
                    ])),
                    true,
                ),
                Field::new("name", DataType::Utf8, true),
                Field::new("entry_type", DataType::Int32, true),
                Field::new("created_at", DataType::Int64, true),
                Field::new("updated_at", DataType::Int64, true),
            ],
            HashMap::default(),
        ))
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let request = ListPartitionsRequest {
            entry: Some(DatasetHandle {
                entry_id: Some(self.tuid.into()),
                dataset_url: Some(self.url.clone()),
            }),
            scan_parameters: None,
        };

        self.client.list_partitions(request).await
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
