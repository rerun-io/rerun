//! High-level Rust APIs for Rerun catalogs.

mod asset;
mod dataset_info;
mod entry;
mod registration;
mod segment;
mod staging;
mod table;

use re_redap_client::{ApiResult, ConnectionClient, ConnectionHandle};

pub use self::asset::{
    Asset, AssetCoverage, AssetLayer, AssetRegistrationError, DEFAULT_ASSET_TASK_TIMEOUT,
    asset_data_source,
};
pub use self::dataset_info::{DatasetInfo, LayerInfo, TimelineInfo};
pub use self::registration::{RegistrationHandle, SegmentRegistrationResult};
pub use self::staging::StagingError;

/// High-level catalog operations backed by a short-lived protocol client per request.
#[derive(Clone, Debug)]
pub struct CatalogClient {
    connection: ConnectionHandle,
}

impl CatalogClient {
    /// A client for the catalog at the origin of `connection`.
    pub fn new(connection: ConnectionHandle) -> Self {
        Self { connection }
    }

    /// Access low-level RPCs and shared connection state for this catalog's origin.
    pub fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }

    /// The server this client talks to.
    pub fn origin(&self) -> &re_uri::Origin {
        self.connection.origin()
    }

    /// A short-lived [`ConnectionClient`] for low-level RPCs against this catalog's origin.
    ///
    /// Acquire a new one per request rather than holding on to it.
    pub async fn client(&self) -> ApiResult<ConnectionClient> {
        self.connection.client().await
    }
}
