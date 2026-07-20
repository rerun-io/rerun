#![cfg(target_arch = "wasm32")]

// NOTE: The end-goal here should be to run the `wasm32` build of the server
// against the `re_redap_tests` conformance suite.

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{
    EntityPath, EntryName, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_protos::cloud::v1alpha1::ext::RegisterWithDatasetDataframe;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService as _;
use re_protos::cloud::v1alpha1::{
    CreateDatasetEntryRequest, DataSource, DataSourceKind, GetDatasetSchemaRequest,
    RegisterWithDatasetRequest, VersionRequest,
};
use re_protos::headers::RerunHeadersInjectorExt as _;
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

    assert_eq!(response.version, re_build_info::exposed_version!());
    assert!(response.build_info.is_some());
}

#[wasm_bindgen_test]
async fn register_rrd_from_file_url_in_opfs() {
    let service = RerunCloudHandlerBuilder::new().build();
    let dataset_name = EntryName::new("opfs_dataset").expect("valid dataset name");
    let file_name = format!("{}.rrd", re_tuid::Tuid::new());
    let url = format!("file:///{file_name}");

    re_server::opfs::write(&file_name, encode_rrd().into())
        .await
        .expect("failed to write OPFS file");

    service
        .create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
            name: Some(dataset_name.as_str().to_owned()),
            id: None,
        }))
        .await
        .expect("failed to create dataset");

    let response = service
        .register_with_dataset(
            tonic::Request::new(RegisterWithDatasetRequest {
                data_sources: vec![DataSource {
                    storage_url: Some(url.clone()),
                    layer: None,
                    prefix: false,
                    typ: DataSourceKind::Rrd as i32,
                }],
                on_duplicate: Default::default(),
            })
            .with_entry_name(dataset_name.clone()),
        )
        .await
        .expect("failed to register OPFS RRD")
        .into_inner();

    let registered: arrow::array::RecordBatch = response
        .data
        .expect("registration response should contain data")
        .try_into()
        .expect("registration response should contain a record batch");
    let registered = RegisterWithDatasetDataframe::try_from(registered)
        .expect("registration response should match its declared schema");
    assert_eq!(
        registered
            .rerun_storage_url
            .into_iter_owned()
            .collect::<Vec<_>>(),
        [url]
    );
    assert_eq!(
        registered
            .rerun_segment_type
            .into_iter_owned()
            .collect::<Vec<_>>(),
        ["rrd"]
    );

    let schema = service
        .get_dataset_schema(
            tonic::Request::new(GetDatasetSchemaRequest {}).with_entry_name(dataset_name),
        )
        .await
        .expect("failed to get dataset schema")
        .into_inner()
        .schema()
        .expect("dataset schema should decode");

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

fn encode_rrd() -> Vec<u8> {
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
