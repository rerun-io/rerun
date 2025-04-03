use std::sync::Arc;

use datafusion::{catalog::TableProvider, error::DataFusionError};
use tonic::transport::Channel;

use crate::partition_table::PartitionTableProvider;
use crate::table_entry_provider::TableEntryTableProvider;
use re_log_types::external::re_tuid::Tuid;
use re_protos::common::v1alpha1::ext::EntryId;
use re_protos::{
    catalog::v1alpha1::{ext::EntryDetails, DatasetEntry, EntryFilter, ReadDatasetEntryRequest},
    frontend::v1alpha1::frontend_service_client::FrontendServiceClient,
};

pub struct DataFusionConnector {
    catalog: FrontendServiceClient<Channel>,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        let catalog = FrontendServiceClient::new(channel.clone());
        Self { catalog }
    }
}

impl DataFusionConnector {
    pub async fn get_entry_list(&mut self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        // TODO(jleibs): Clean this up with better helpers
        let entry: EntryDetails = self
            .catalog
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

        TableEntryTableProvider::new(self.catalog.clone(), entry.id)
            .into_provider()
            .await
    }

    pub async fn get_dataset_entry(
        &mut self,
        id: Tuid,
    ) -> Result<Option<DatasetEntry>, tonic::Status> {
        let entry = self
            .catalog
            .read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(id.into()),
            })
            .await?
            .into_inner()
            .dataset;

        Ok(entry)
    }

    pub async fn get_partition_table(
        &self,
        dataset_id: EntryId,
    ) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        PartitionTableProvider::new(self.catalog.clone(), dataset_id)
            .into_provider()
            .await
    }
}
