use std::sync::Arc;

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};

use re_grpc_client::ConnectionClient;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{EntryId, EntryIdOrName};
use re_protos::catalog::v1alpha1::ext::EntryDetails;
use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind, FindEntriesRequest};
use re_protos::frontend::v1alpha1::{GetTableSchemaRequest, ScanTableRequest, ScanTableResponse};

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Debug, Clone)]
pub struct TableEntryTableProvider {
    //TODO(#10191): this should use a `ConnectionRegistryHandle` instead
    client: ConnectionClient,
    table: EntryIdOrName,

    // cache the table id when resolved
    table_id: Option<EntryId>,
}

impl TableEntryTableProvider {
    pub fn new(client: ConnectionClient, table: impl Into<EntryIdOrName>) -> Self {
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
        if let Some(table_id) = self.table_id {
            return Ok(table_id);
        }

        let table_id = match &self.table {
            EntryIdOrName::Id(entry_id) => *entry_id,

            EntryIdOrName::Name(table_name) => {
                let mut client = self.client.clone();
                let table_name_copy = table_name.clone();

                let entry_details: EntryDetails = make_future_send(async move {
                    Ok(client
                        .inner()
                        .find_entries(FindEntriesRequest {
                            filter: Some(EntryFilter {
                                id: None,
                                name: Some(table_name_copy),
                                entry_kind: Some(EntryKind::Table as i32),
                            }),
                        })
                        .await)
                })
                .await?
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

                entry_details.id
            }
        };

        self.table_id = Some(table_id);
        Ok(table_id)
    }
}

#[async_trait]
impl GrpcStreamToTable for TableEntryTableProvider {
    type GrpcStreamData = ScanTableResponse;

    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        let mut client = self.client.clone();

        Ok(Arc::new(
            make_future_send(async move { Ok(client.inner().get_table_schema(request).await) })
                .await?
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

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().scan_table(request).await) })
            .await?
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
