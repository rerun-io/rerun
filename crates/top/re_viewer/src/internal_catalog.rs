//! The in-process "internal catalog" [`re_server`].
//!
//! The app hosts a single in-process [`re_server`] (the "internal catalog").
//! The viewer then loads local resources by registering them with that catalog and opening the
//! resulting redap segment URI, instead of importing them directly.
//!
//! The viewer talks to the catalog in-process via [`InternalCatalog::connection`].
//! On native, the same handler and its upload route are also served on the proxy server's port so
//! that other local processes can reach it.
//! The served endpoint is restricted to connections from the local machine.

#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
use std::sync::Arc;

use re_redap_client::Connection;
use re_server::RerunCloudHandlerBuilder;

#[cfg(not(target_arch = "wasm32"))]
use {
    re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer,
    re_server::RerunCloudHandler,
};

/// The in-process internal catalog.
pub struct InternalCatalog {
    /// The in-process connection the viewer uses to talk to the catalog.
    pub connection: Connection,

    /// The single handler shared between [`Self::connection`] and [`Self::grpc_service`].
    #[cfg(not(target_arch = "wasm32"))]
    handler: Arc<RerunCloudHandler>,

    #[cfg(not(target_arch = "wasm32"))]
    write_upload_route: axum::routing::MethodRouter,

    storage_dir: std::path::PathBuf,
}

impl InternalCatalog {
    /// The origin under which the catalog is registered.
    pub fn origin(&self) -> &re_uri::Origin {
        self.connection.origin()
    }

    /// The filesystem root used for objects staged for this catalog.
    pub fn storage_dir(&self) -> &std::path::Path {
        &self.storage_dir
    }

    /// The catalog as a gRPC service, to be served (loopback-only) on the proxy server's port.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn grpc_service(&self) -> RerunCloudServiceServer<RerunCloudHandler> {
        RerunCloudServiceServer::from_arc(self.handler.clone())
            .max_decoding_message_size(re_redap_client::MAX_DECODING_MESSAGE_SIZE)
    }

    /// The HTTP endpoint for redeeming write grants.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_upload_route(&self) -> axum::routing::MethodRouter {
        self.write_upload_route.clone()
    }
}

/// Build the in-process internal catalog, addressed at the proxy server's port.
#[cfg(not(target_arch = "wasm32"))]
pub fn build(proxy_addr: SocketAddr) -> InternalCatalog {
    let origin = re_uri::Origin::http_local_host(proxy_addr.port());
    let storage_dir = tempfile::Builder::new()
        .prefix("rerun-data-")
        .tempdir()
        .expect("failed to create internal catalog storage directory");
    let storage_path = storage_dir.path().to_owned();
    let (handler, write_upload_route) = RerunCloudHandlerBuilder::new()
        .with_storage_dir(storage_dir)
        .build_with_write_access()
        .expect("failed to build internal catalog");
    let handler = Arc::new(handler);
    let connection = Connection::from_service(origin, handler.clone(), re_server::capabilities());

    InternalCatalog {
        connection,
        handler,
        write_upload_route,
        storage_dir: storage_path,
    }
}

/// Build the in-process internal catalog.
#[cfg(target_arch = "wasm32")]
pub fn build() -> InternalCatalog {
    let handler = Arc::new(RerunCloudHandlerBuilder::new().build());

    // The Wasm catalog lives purely in-process; the loopback address is a stable identity for the
    // in-memory handler (matching the native construction), not a reachable endpoint.
    let origin = re_uri::Origin::http_local_host(0);

    let connection = Connection::from_service(origin, handler, re_server::capabilities());

    InternalCatalog {
        connection,
        storage_dir: re_web::fs::root(),
    }
}
