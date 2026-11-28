//! A client names the store to register only to a server at the protocol level that registers one.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;
use std::time::Duration;

use re_catalog_client::CatalogClient;
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_protos::protocol_level::ProtocolLevel;
use re_redap_client::{ApiErrorKind, Connection, ConnectionRegistry};
use re_server::{RerunCloudHandlerBuilder, ServerBuilder};

/// Writes an RRD of two stores that share a recording id to `dir`, and returns a data source that
/// names the second one.
fn second_store_of_a_two_store_rrd(dir: &std::path::Path) -> DataSource {
    let (path, store_ids) = re_redap_tests::create_recording_with_shared_recording_id_in(
        740,
        "shared_recording",
        &["app_one", "app_two"],
        dir,
    )
    .expect("recording");
    let url = url::Url::from_file_path(&path).expect("absolute path");
    DataSource::new_rrd_url(url, None).with_store_id(store_ids[1].clone())
}

fn runtime() -> re_async::AsyncRuntimeHandle {
    re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test runtime is running")
}

/// A server below `REGISTER_STORE_ID` ignores `DataSource.store_id` and registers another store,
/// so the client must refuse to send one to it.
#[tokio::test(flavor = "multi_thread")]
async fn client_refuses_to_name_a_store_to_a_server_below_the_level() {
    let dir = tempfile::tempdir().expect("temp dir");
    let data_source = second_store_of_a_two_store_rrd(dir.path());

    let handler = RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!())
        .build()
        .expect("failed to build handler");
    let origin = re_uri::Origin::http_local_host(1);
    let registry = ConnectionRegistry::new_without_stored_credentials().with_internal(
        Connection::from_service(
            origin.clone(),
            Arc::new(handler),
            re_server::capabilities(),
            ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST,
        ),
        dir.path().to_path_buf(),
    );
    let catalog = CatalogClient::new(registry.connection_handle(origin), runtime());

    let dataset_name = EntryName::new("my_dataset").expect("valid entry name");
    let dataset_id = catalog
        .find_or_create_dataset(&dataset_name)
        .await
        .expect("dataset is created");
    let Err(err) = catalog
        .register_with_dataset(dataset_id, vec![data_source], IfDuplicateBehavior::Error)
        .await
    else {
        panic!("the client must refuse to name a store to this server");
    };
    assert_eq!(err.kind, ApiErrorKind::Unimplemented, "{err}");
    assert!(
        catalog
            .get_dataset_segment_ids(dataset_id)
            .await
            .expect("segments are listed")
            .is_empty(),
        "the refused request must not reach the server"
    );
}

/// The client learns the protocol level of a remote server from its `/WhoAmI`, and the server
/// advertises one at which it registers the named store.
#[tokio::test(flavor = "multi_thread")]
async fn client_names_a_store_to_a_remote_server() {
    let dir = tempfile::tempdir().expect("temp dir");
    let data_source = second_store_of_a_two_store_rrd(dir.path());

    let handler = RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!())
        .build()
        .expect("failed to build handler");
    let server = ServerBuilder::bind((std::net::Ipv4Addr::LOCALHOST, 0).into())
        .expect("bind")
        .with_service(RerunCloudServiceServer::new(handler))
        .build()
        .start(&runtime())
        .await
        .expect("server starts");
    let origin = format!("rerun+http://{}", server.connect_addr())
        .parse()
        .expect("origin");
    let catalog = CatalogClient::new(
        ConnectionRegistry::new_without_stored_credentials().connection_handle(origin),
        runtime(),
    );

    let dataset_name = EntryName::new("my_dataset").expect("valid entry name");
    let dataset_id = catalog
        .find_or_create_dataset(&dataset_name)
        .await
        .expect("dataset is created");
    let segment_ids = catalog
        .register_with_dataset(dataset_id, vec![data_source], IfDuplicateBehavior::Error)
        .await
        .expect("registration is accepted")
        .wait(Duration::from_secs(60))
        .await
        .expect("registration succeeds");
    assert_eq!(segment_ids, [SegmentId::from("shared_recording")]);
}
