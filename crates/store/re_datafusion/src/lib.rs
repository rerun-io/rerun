//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::sync::Arc;

use catalog_find_entries::CatalogFindEntryProvider;
use datafusion::catalog::TableProvider;
use partition_index_list::PartitionIndexListProvider;
use partition_list::PartitionListProvider;
use re_log_types::external::re_tuid::Tuid;
use re_protos::catalog::v1alpha1::{
    catalog_service_client::CatalogServiceClient, DatasetEntry, EntryKind, ReadDatasetEntryRequest,
};
use tonic::transport::Channel;

pub mod catalog_find_entries;
pub mod grpc_response_provider;
pub mod grpc_streaming_provider;
pub mod partition_index_list;
pub mod partition_list;
pub mod table_entry_provider;

pub struct DataFusionConnector {
    catalog: CatalogServiceClient<Channel>,
    channel: Channel,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        let catalog = CatalogServiceClient::new(channel.clone());
        Self {
            catalog,
            channel: channel.clone(),
        }
    }
}

impl DataFusionConnector {
    pub fn get_all_datasets(&self) -> Arc<dyn TableProvider> {
        CatalogFindEntryProvider::new(self.catalog.clone(), None, None, Some(EntryKind::Dataset))
            .into_provider()
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

    pub async fn get_partition_list(&self, tuid: Tuid, url: &str) -> Arc<dyn TableProvider> {
        PartitionListProvider::new(self.channel.clone(), tuid, url)
            .into_provider()
            .await
    }

    pub async fn get_partition_index_list(&self, tuid: Tuid, url: &str) -> Arc<dyn TableProvider> {
        PartitionIndexListProvider::new(self.channel.clone(), tuid, url)
            .into_provider()
            .await
    }
}
