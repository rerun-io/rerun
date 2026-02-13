use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::TableProvider;
use datafusion::error::{DataFusionError, Result as DataFusionResult};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{SearchDatasetRequest, SearchDatasetResponse};
use re_protos::common::v1alpha1::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::ConnectionClient;
use tokio_stream::StreamExt as _;
use tracing::instrument;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Clone)]
pub struct SearchResultsTableProvider {
    client: ConnectionClient,
    dataset_id: EntryId,
    request: SearchDatasetRequest,
}

impl std::fmt::Debug for SearchResultsTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchResultsTableProvider")
            .field("request", &self.request)
            .finish()
    }
}

impl SearchResultsTableProvider {
    pub fn new(
        client: ConnectionClient,
        dataset_id: EntryId,
        request: SearchDatasetRequest,
    ) -> Result<Self, DataFusionError> {
        if request.scan_parameters.is_some() {
            return Err(DataFusionError::External(
                "Scan parameters are not supported for SearchResultsTableProvider".into(),
            ));
        }

        Ok(Self {
            client,
            dataset_id,
            request,
        })
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for SearchResultsTableProvider {
    type GrpcStreamData = SearchDatasetResponse;

    #[instrument(skip(self), err)]
    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let mut request = self.request.clone();
        request.scan_parameters = Some(ScanParameters {
            limit_len: Some(0),
            ..Default::default()
        });

        let mut client = self.client.clone();
        let dataset_id = self.dataset_id;

        let rb: RecordBatch = make_future_send(async move {
            Ok::<_, DataFusionError>(
                client
                    .inner()
                    .search_dataset(
                        tonic::Request::new(request)
                            .with_entry_id(dataset_id)
                            .map_err(|err| DataFusionError::External(Box::new(err)))?,
                    )
                    .await
                    .map_err(|err| DataFusionError::External(Box::new(err)))?
                    .into_inner()
                    .next()
                    .await,
            )
        })
        .await?
        .ok_or(DataFusionError::Execution(
            "Empty stream from search results".to_owned(),
        ))?
        .map_err(|err| DataFusionError::External(Box::new(err)))?
        .data
        .ok_or(DataFusionError::Execution(
            "Empty data from search results".to_owned(),
        ))?
        .try_into()
        .map_err(|err| DataFusionError::External(Box::new(err)))?;

        Ok(rb.schema())
    }

    #[instrument(skip(self), err)]
    async fn send_streaming_request(
        &mut self,
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>> {
        let request = tonic::Request::new(self.request.clone())
            .with_entry_id(self.dataset_id)
            .map_err(|err| DataFusionError::External(Box::new(err)))?;

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().search_dataset(request).await) })
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
                "DataFrame missing from SearchDataResponse response".to_owned(),
            ))?
            .try_into()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }
}
