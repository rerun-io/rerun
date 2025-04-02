use std::{collections::HashMap, sync::Arc};

use arrow_flight::{
    flight_service_client::FlightServiceClient, utils::flight_data_to_arrow_batch, FlightData,
    Ticket,
};
use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    common::{exec_datafusion_err, exec_err},
    error::{DataFusionError, Result as DataFusionResult},
};
use futures::StreamExt;
use tonic::transport::Channel;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct FlightResponseProvider {
    pub(crate) schema: SchemaRef,
    pub(crate) client: FlightServiceClient<Channel>,
    pub(crate) ticket: Option<Ticket>,
    pub(crate) schema_ticket: Option<Ticket>,
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
        let Some(ticket) = self.schema_ticket.take() else {
            return exec_err!("no schema ticket data");
        };

        let flight_data = self
            .client
            .do_get(ticket)
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner()
            .next()
            .await
            .ok_or_else(|| exec_datafusion_err!("No response to schema request"))?
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let message = arrow::ipc::root_as_message(&flight_data.data_header[..]).map_err(|_| {
            arrow::error::ArrowError::CastError("Cannot get root as message".to_string())
        })?;

        let Some(ipc_schema) = message.header_as_schema() else {
            exec_err!("Unable to retrieve schema in flight data")?
        };

        let schema = arrow::ipc::convert::fb_to_schema(ipc_schema);
        self.schema = Arc::new(schema);

        return Ok(Arc::clone(&self.schema));
    }

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status> {
        let Some(ticket) = self.ticket.take() else {
            return Err(tonic::Status::aborted("no ticket data"));
        };

        self.client.do_get(ticket).await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        // println!("PROCESS RESPONSE");
        let message = arrow::ipc::root_as_message(&response.data_header[..]).map_err(|_| {
            arrow::error::ArrowError::CastError("Cannot get root as message".to_string())
        })?;

        if message.header_as_schema().is_some() {
            // This flight info was for the schema, so we can return an empty batch
            return Ok(RecordBatch::new_empty(Arc::clone(&self.schema)));
        }

        let dictionaries_by_id = HashMap::new();
        flight_data_to_arrow_batch(&response, Arc::clone(&self.schema), &dictionaries_by_id)
            .map_err(DataFusionError::from)
    }
}
