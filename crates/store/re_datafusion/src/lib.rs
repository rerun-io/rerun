//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::sync::Arc;

use datafusion::catalog::TableProvider;
use partition_index_list::PartitionIndexListProvider;
use partition_list::PartitionListProvider;
use re_log_types::external::re_tuid::Tuid;
use re_protos::{
    catalog::v1alpha1::{ext::EntryDetails, DatasetEntry, EntryFilter, ReadDatasetEntryRequest},
    frontend::v1alpha1::frontend_service_client::FrontendServiceClient,
};
use table_entry_provider::TableEntryProvider;
use tonic::transport::Channel;

pub mod grpc_response_provider;
pub mod grpc_streaming_provider;
pub mod partition_index_list;
pub mod partition_list;
pub mod table_entry_provider;

pub struct DataFusionConnector {
    catalog: FrontendServiceClient<Channel>,
    channel: Channel,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        let catalog = FrontendServiceClient::new(channel.clone());
        Self {
            catalog,
            channel: channel.clone(),
        }
    }
}

impl DataFusionConnector {
    pub async fn get_entry_list(&mut self) -> Arc<dyn TableProvider> {
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
            .unwrap()
            .into_inner()
            .entries
            .into_iter()
            .next()
            .unwrap()
            .try_into()
            .unwrap();

        TableEntryProvider::new(self.catalog.clone(), entry.id)
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
