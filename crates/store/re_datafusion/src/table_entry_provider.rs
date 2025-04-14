use std::sync::Arc;

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use tonic::transport::Channel;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{EntryId, EntryIdOrName};
use re_protos::catalog::v1alpha1::ext::EntryDetails;
use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind, FindEntriesRequest};
use re_protos::frontend::v1alpha1::{
    frontend_service_client::FrontendServiceClient, GetTableSchemaRequest, ScanTableRequest,
    ScanTableResponse,
};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};

#[derive(Debug, Clone)]
pub struct TableEntryTableProvider {
    client: FrontendServiceClient<Channel>,
    table: EntryIdOrName,

    // cache the table id when resolved
    table_id: Option<EntryId>,
}

impl TableEntryTableProvider {
    pub fn new(client: FrontendServiceClient<Channel>, table: impl Into<EntryIdOrName>) -> Self {
        Self {
            client,
            table: table.into(),
            table_id: None,
        }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }

    async fn table_id(&mut self) -> Result<EntryId, DataFusionError> {
        match &self.table {
            EntryIdOrName::Id(entry_id) => Ok(*entry_id),

            EntryIdOrName::Name(table_name) => {
                let entry_details: EntryDetails = self
                    .client
                    .find_entries(FindEntriesRequest {
                        filter: Some(EntryFilter {
                            id: None,
                            name: Some(table_name.clone()),
                            entry_kind: Some(EntryKind::Table as i32),
                        }),
                    })
                    .await
                    .map_err(|err| DataFusionError::External(Box::new(err)))?
                    .into_inner()
                    .entries
                    .first()
                    .ok_or_else(|| {
                        DataFusionError::External(
                            format!("No entry found with name: {table_name}").into(),
                        )
                    })?
                    .clone()
                    .try_into()
                    .map_err(|err| DataFusionError::External(Box::new(err)))?;

                Ok(entry_details.id)
            }
        }
    }
}

#[async_trait]
impl GrpcStreamToTable for TableEntryTableProvider {
    type GrpcStreamData = ScanTableResponse;

    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id().await?.into()),
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
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>> {
        let request = ScanTableRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        self.client
            .scan_table(request)
            .await
            .map_err(|err| DataFusionError::External(Box::new(err)))
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
