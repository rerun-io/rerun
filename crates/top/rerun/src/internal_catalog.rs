//! The in-process "internal catalog" [`re_server`].
//!
//! The app hosts a single in-process [`re_server`] (the "internal catalog") and registers
//! its connection client with the [`ConnectionRegistryHandle`]. The viewer then loads local
//! resources by registering them with that catalog and opening the resulting redap segment URI,
//! instead of importing them directly.

use std::net::{Ipv4Addr, SocketAddr};

use anyhow::Context as _;

use re_redap_client::ConnectionRegistryHandle;
use re_server::{RerunCloudHandlerBuilder, ServerBuilder, ServerHandle};

/// Start the internal catalog.
pub async fn start(
    connection_registry: ConnectionRegistryHandle,
) -> anyhow::Result<(ServerHandle, re_uri::Origin)> {
    let handler = RerunCloudHandlerBuilder::new().build();
    let rerun_cloud_service =
        re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer::new(
            handler,
        );

    // Bind the default redap port on loopback.
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, re_uri::DEFAULT_REDAP_PORT));

    let server_handle = ServerBuilder::default()
        .with_address(addr)
        .with_service(rerun_cloud_service)
        .build()
        .start()
        .await
        .context("failed to start the internal catalog")?;

    let origin = re_uri::Origin::from_scheme_and_socket_addr(
        re_uri::Scheme::RerunHttp,
        server_handle.connect_addr(),
    );

    // Warm up (and validate) the connection. The client is cached in the registry, so the viewer
    // gets it for free when it later loads a file via this origin.
    connection_registry
        .client(origin.clone())
        .await
        .context("failed to connect to the internal catalog")?;

    re_log::debug!("internal catalog server listening at {origin}");

    Ok((server_handle, origin))
}
