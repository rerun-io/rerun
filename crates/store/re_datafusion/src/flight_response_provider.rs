use std::{collections::HashMap, sync::Arc};

use arrow_flight::{
    flight_service_client::FlightServiceClient, utils::flight_data_to_arrow_batch, FlightData,
    Ticket,
};
use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tonic::transport::Channel;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct FlightResponseProvider {
    pub(crate) schema: SchemaRef,
    pub(crate) client: FlightServiceClient<Channel>,
    pub(crate) ticket: Option<Ticket>,
}

impl FlightResponseProvider {
    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for FlightResponseProvider {
    type GrpcStreamData = FlightData;

    async fn fetch_schema(&mut self) -> Result<SchemaRef, DataFusionError> {
        Ok(Arc::clone(&self.schema))
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let ticket = self
            .ticket
            .take()
            .unwrap_or(Err(tonic::Status::aborted("no ticket data"))?);
        self.client.do_get(ticket).await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        let dictionaries_by_id = HashMap::new();
        flight_data_to_arrow_batch(&response, Arc::clone(&self.schema), &dictionaries_by_id)
            .map_err(Into::into)
    }
}
