use std::sync::Arc;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use async_trait::async_trait;
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tokio_stream::StreamExt as _;
use tonic::transport::Channel;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    common::v1alpha1::ScanParameters,
    frontend::v1alpha1::{frontend_service_client::FrontendServiceClient, SearchDatasetRequest},
    manifest_registry::v1alpha1::SearchDatasetResponse,
};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct SearchResultsTableProvider {
    client: FrontendServiceClient<Channel>,
    request: SearchDatasetRequest,
}

impl SearchResultsTableProvider {
    pub fn new(
        client: FrontendServiceClient<Channel>,
        request: SearchDatasetRequest,
    ) -> Result<Self, DataFusionError> {
        if request.scan_parameters.is_some() {
            return Err(DataFusionError::External(
                "Scan parameters are not supported for SearchResultsTableProvider".into(),
            ));
        }

        Ok(Self { client, request })
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for SearchResultsTableProvider {
    type GrpcStreamData = SearchDatasetResponse;

    async fn fetch_schema(&mut self) -> Result<SchemaRef, DataFusionError> {
        let mut request = self.request.clone();
        request.scan_parameters = Some(ScanParameters {
            limit_len: Some(0),
            ..Default::default()
        });

        let schema = self
            .client
            .search_dataset(request)
            .await
            .map_err(|err| DataFusionError::External(Box::new(err)))?
            .into_inner()
            .next()
            .await
            .ok_or(DataFusionError::Execution(
                "Empty stream from search results".to_owned(),
            ))?
            .map_err(|err| DataFusionError::External(Box::new(err)))?
            .data
            .ok_or(DataFusionError::Execution(
                "Empty data from search results".to_owned(),
            ))?
            .decode()
            .map_err(|err| DataFusionError::External(Box::new(err)))?
            .schema();

        Ok(schema)
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let request = self.request.clone();

        self.client.search_dataset(request).await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        response
            .data
            .ok_or(DataFusionError::Execution(
                "DataFrame missing from SearchDataResponse response".to_owned(),
            ))?
            .decode()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }
}
