use std::sync::Arc;

use datafusion::{catalog::TableProvider, error::DataFusionError};
use tracing::instrument;

use re_grpc_client::ConnectionClient;
use re_log_types::{EntryId, external::re_tuid::Tuid};
use re_protos::catalog::v1alpha1::{
    DatasetEntry, EntryFilter, ReadDatasetEntryRequest, ext::EntryDetails,
};

use crate::partition_table::PartitionTableProvider;
use crate::table_entry_provider::TableEntryTableProvider;

pub struct DataFusionConnector {
    //TODO(#10191): this should hold on to a `ConnectionRegistryHandle` instead of a `ConnectionClient`
    client: ConnectionClient,
}

impl DataFusionConnector {
    pub async fn new(client: ConnectionClient) -> anyhow::Result<Self> {
        Ok(Self { client })
    }
}

impl DataFusionConnector {
    #[instrument(skip_all, err)]
    pub async fn get_entry_list(&mut self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        // TODO(jleibs): Clean this up with better helpers
        let entry: EntryDetails = self
            .client
            .inner()
            .find_entries(re_protos::catalog::v1alpha1::FindEntriesRequest {
                filter: Some(EntryFilter {
                    name: Some("__entries".to_owned()),
                    ..Default::default()
                }),
            })
            .await
            .map_err(|err| DataFusionError::External(Box::new(err)))?
            .into_inner()
            .entries
            .into_iter()
            .next()
            .ok_or(DataFusionError::External("No __entries table found".into()))?
            .try_into()
            .map_err(|err| DataFusionError::External(Box::new(err)))?;

        TableEntryTableProvider::new(self.client.clone(), entry.id)
            .into_provider()
            .await
    }

    #[instrument(skip(self), err)]
    pub async fn get_dataset_entry(
        &mut self,
        id: Tuid,
    ) -> Result<Option<DatasetEntry>, tonic::Status> {
        let entry = self
            .client
            .inner()
            .read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(id.into()),
            })
            .await?
            .into_inner()
            .dataset;

        Ok(entry)
    }

    #[instrument(skip(self), err)]
    pub async fn get_partition_table(
        &self,
        dataset_id: EntryId,
    ) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        PartitionTableProvider::new(self.client.clone(), dataset_id)
            .into_provider()
            .await
    }
}
