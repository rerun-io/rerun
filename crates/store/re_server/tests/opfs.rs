#![cfg(target_arch = "wasm32")]

// NOTE: The end-goal here should be to run the `wasm32` build of the server
// against the `re_redap_tests` conformance suite.

use std::sync::Arc;
use std::time::Duration;

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{
    EntityPath, EntryName, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_protos::cloud::v1alpha1::VersionRequest;
use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{Connection, ConnectionHandle, ConnectionRegistry};
use re_server::RerunCloudHandlerBuilder;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn version() {
    let service = RerunCloudHandlerBuilder::new().build();

    let response = service
        .version(tonic::Request::new(VersionRequest {}))
        .await
        .expect("version request should succeed")
        .into_inner();

    assert_eq!(response.version, re_build_info::exposed_version());
    assert!(response.build_info.is_some());
}

#[wasm_bindgen_test]
async fn register_rrd_with_footer_from_file_url_in_opfs() {
    register_rrd_from_file_url_in_opfs(true).await;
}

#[wasm_bindgen_test]
async fn register_rrd_without_footer_from_file_url_in_opfs() {
    register_rrd_from_file_url_in_opfs(false).await;
}

/// Serve `service` in-process as the internal origin of a fresh connection registry.
///
/// The placeholder origin is never dialed; requests go straight to `service`.
fn in_process_connection<T: RerunCloudService>(service: Arc<T>) -> ConnectionHandle {
    let registry = ConnectionRegistry::new_without_stored_credentials();
    registry.set_internal(Connection::from_service(
        re_uri::Origin::http_local_host(1),
        service,
    ));
    registry
        .internal_connection_handle()
        .expect("internal connection is configured")
}

async fn register_rrd_from_file_url_in_opfs(with_footer: bool) {
    let service = Arc::new(RerunCloudHandlerBuilder::new().build());
    let connection = in_process_connection(service);
    let mut client = connection.client().await.expect("failed to get client");
    let footer_suffix = if with_footer {
        "with_footer"
    } else {
        "without_footer"
    };
    let dataset_name =
        EntryName::new(format!("opfs_dataset_{footer_suffix}")).expect("valid dataset name");
    let file_name = format!("{}.rrd", re_tuid::Tuid::new());
    let url = format!("file:///{file_name}");

    re_web::fs::write(&file_name, encode_rrd(with_footer).into())
        .await
        .expect("failed to write OPFS file");

    let dataset = client
        .create_dataset_entry(dataset_name, None)
        .await
        .expect("failed to create dataset");
    let registration = connection
        .register_with_dataset(
            dataset.details.id,
            vec![DataSource::new_rrd(url).expect("valid OPFS URL")],
            IfDuplicateBehavior::Error,
        )
        .await
        .expect("failed to register OPFS RRD");
    let segment_ids = registration
        .wait(Duration::from_secs(10))
        .await
        .expect("failed to wait for OPFS RRD registration");
    assert_eq!(segment_ids.len(), 1);

    let schema = client
        .get_dataset_schema(dataset.details.id)
        .await
        .expect("failed to get dataset schema");

    assert!(schema.fields().iter().any(|field| {
        let metadata = field.metadata();
        metadata
            .get("rerun:entity_path")
            .is_some_and(|path| path == "/test/entity")
            && metadata
                .get("rerun:component")
                .is_some_and(|component| component == "example.MyPoints:points")
    }));
}

fn encode_rrd(with_footer: bool) -> Vec<u8> {
    let store_id = StoreId::random(StoreKind::Recording, "opfs_test");
    let timeline = Timeline::new_sequence("frame");
    let points = MyPoint::from_iter(0..1);
    let chunk = Chunk::builder(EntityPath::from("/test/entity"))
        .with_sparse_component_batches(
            RowId::new(),
            TimePoint::default().with(timeline, 0),
            [(MyPoints::descriptor_points(), Some(&points as _))],
        )
        .build()
        .expect("test chunk should be valid");

    let mut bytes = Vec::new();
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_build_info::CrateVersion::LOCAL,
        re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED,
        &mut bytes,
    )
    .expect("failed to create test RRD encoder");
    if !with_footer {
        encoder.do_not_emit_footer();
    }
    encoder
        .append(&LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        }))
        .expect("failed to write test store info");
    encoder
        .append(&LogMsg::ArrowMsg(
            store_id,
            chunk
                .to_arrow_msg()
                .expect("test chunk should encode as arrow"),
        ))
        .expect("failed to write test chunk");
    encoder.finish().expect("failed to finish test RRD");
    drop(encoder);
    bytes
}
