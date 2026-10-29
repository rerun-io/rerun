//! High-level Rust APIs for Rerun catalogs, shared by the SDK and viewer.
//!
//! Consider both the Python SDK (`rerun_py`) and the viewer when adding features so the API serves both well.
//! Keep this crate focused on generally applicable catalog operations rather than single-use helpers.
//!
//! Downstream crates are encouraged to extend `CatalogClient` with their own extension traits.
//! As common needs emerge, those features can be generalized into `re_catalog_client`.
//!
//! Give this crate the same API design and review scrutiny as the crates in `crates/top`, since it may move there in the future.

mod asset;
mod dataset_info;
mod dataset_selection;
mod entry;
mod registration;
mod segment;
mod staging;
mod table;
mod write;

use re_redap_client::{ApiResult, ConnectionClient, ConnectionHandle};

pub use self::asset::{
    Asset, AssetCoverage, AssetLayer, AssetRegistrationError, DEFAULT_ASSET_TASK_TIMEOUT,
    RegisteredAssets, asset_data_source,
};
pub use self::dataset_info::{DatasetInfo, LayerInfo, TimelineInfo};
pub use self::dataset_selection::{DatasetSelection, SegmentSelection};
pub use self::registration::{RegistrationError, RegistrationHandle, SegmentRegistrationResult};
pub use self::staging::{StageOpt, StagingError};
pub use self::write::{WriteError, WriteHandle};

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
