//! A client learns the protocol level of a server when it connects to it.

#![cfg(not(target_arch = "wasm32"))]

use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;
use re_redap_client::ConnectionRegistry;
use re_server::{RerunCloudHandlerBuilder, ServerBuilder};

/// The server advertises its protocol level on `/WhoAmI`, the call a client makes to connect.
#[tokio::test(flavor = "multi_thread")]
async fn client_reads_the_protocol_level_a_server_advertises() {
    let runtime = re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test runtime is running");
    let handler = RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!())
        .build()
        .expect("failed to build handler");
    let server = ServerBuilder::bind((std::net::Ipv4Addr::LOCALHOST, 0).into())
        .expect("bind")
        .with_service(RerunCloudServiceServer::new(handler))
        .build()
        .start(&runtime)
        .await
        .expect("server starts");
    let origin = format!("rerun+http://{}", server.connect_addr())
        .parse()
        .expect("origin");

    let connection = ConnectionRegistry::new_without_stored_credentials()
        .connection_handle(origin)
        .connection()
        .await
        .expect("connection is made");

    assert_eq!(
        connection.server_protocol_level(),
        re_server::PROTOCOL_LEVEL
    );
}
