//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::sync::Arc;

use catalog_find_entries::CatalogFindEntryProvider;
use datafusion::catalog::TableProvider;
use partition_index_list::PartitionIndexListProvider;
use partition_list::PartitionListProvider;
use re_log_types::external::re_tuid::Tuid;
use re_protos::catalog::v1alpha1::{catalog_service_client::CatalogServiceClient, EntryKind};
use tonic::transport::Channel;

pub mod catalog_find_entries;
pub mod grpc_response_provider;
pub mod grpc_streaming_provider;
pub mod partition_index_list;
pub mod partition_list;

pub struct DataFusionConnector {
    // catalog: CatalogServiceClient<Channel>,
    channel: Channel,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        Self {
            channel: channel.clone(),
        }
    }
}

impl DataFusionConnector {
    pub fn get_all_datasets(&self) -> Arc<dyn TableProvider> {
        let client = CatalogServiceClient::new(self.channel.clone());

        CatalogFindEntryProvider::new(client, None, None, Some(EntryKind::Dataset)).into_provider()
    }

    pub fn get_partition_list(&self, tuid: Tuid) -> Arc<dyn TableProvider> {
        PartitionListProvider::new(self.channel.clone(), tuid).into_provider()
    }

    pub fn get_partition_index_list(&self, tuid: Tuid) -> Arc<dyn TableProvider> {
        PartitionIndexListProvider::new(self.channel.clone(), tuid).into_provider()
    }
}
