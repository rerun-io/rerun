//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::sync::Arc;


use datafusion::catalog::TableProvider;
use re_protos::catalog::v1alpha1::catalog_service_client::CatalogServiceClient;
use tonic::transport::Channel;

mod dataset_catalog_provider;
mod grpc_table_provider;

use grpc_table_provider::GrpcTableProvider;

pub struct DataFusionConnector {
    catalog: CatalogServiceClient<Channel>,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        let catalog = CatalogServiceClient::new(channel.clone());
        Self { catalog }
    }
}

impl DataFusionConnector {
    pub fn get_datasets(&self) -> Arc<dyn TableProvider> {
        let table_provider: GrpcTableProvider<CatalogServiceClient<Channel>> =
            self.catalog.clone().into();

        Arc::new(table_provider)
    }
}
