//! Low-level protocol requests and response decoding.
//!
//! [`RedapClient`] is defined here, and its methods in the submodules, one per area of the API, so
//! that they can all reach its private fields.

use std::sync::Arc;

use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use tokio::sync::OnceCell;

mod assets;
mod chunks;
mod client;
mod connection;
mod datasets;
mod entries;
mod maintenance;
mod rrd_manifest;
mod schemas;
mod segment_properties;
mod segments_schema;
mod server_info;
mod tables;
mod tasks;
mod uploads;

pub use self::chunks::FetchChunksResponseStream;
pub use self::connection::{BoxedRedapClientStack, Connection, ConnectionClient};
pub use self::datasets::{DatasetManifest, DatasetSchema};
pub use self::segments_schema::OnMissingGetSchemas;

/// Low-level API over the gRPC client.
///
/// Implementation note: this type is generic so that it can be used with several client types. This
/// is useful for other projects which might have a different type (e.g. due to instrumentation).
/// Applications that need lower-level APIs should call [`crate::ConnectionHandle::client`]
/// to acquire a short-lived RPC client, or access the handle through
/// `re_catalog_client::CatalogClient::connection` when using catalog workflows.
//TODO(ab): this should NOT be `Clone`, to discourage callsites from holding on to a client for too
//long. However we have a bunch of places that needs to be fixed before we can do that.
#[derive(Clone)]
pub struct RedapClient<T> {
    /// The server this client talks to, named in every error it produces.
    origin: re_uri::Origin,

    inner: RerunCloudServiceClient<T>,

    /// Cached `VersionResponse.features` list. Populated lazily on the first
    /// `supports_feature` call and reused for the lifetime of this client
    /// (and any clones — `Arc` ensures clones share the same cache).
    ///
    /// Server features are stable per connection; if the server restarts
    /// with a different feature set, callers reconnect and get a fresh
    /// client (and a fresh cache).
    features: Arc<OnceCell<Vec<String>>>,

    /// Cache of certain chunks, like asset chunks.
    ///
    /// `None` disables chunk caching.
    chunk_cache: Option<crate::ChunkCacheHandle>,

    /// Layer schemas looked up by [`Self::get_schemas`].
    schema_cache: crate::schema_cache::SchemaCacheHandle,
}
