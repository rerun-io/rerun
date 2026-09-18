//! Client-side write grants against a running `re_server`.
//!
//! Exercises the intended API end to end: acquire a grant, redeem it with an in-memory RRD, then
//! register the credential-free storage URL with a dataset.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{EntityPath, EntryName, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext::{DataSource, ObjectKey};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionRegistry;
use re_server::{RerunCloudHandlerBuilder, ServerBuilder};

const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test(flavor = "multi_thread")]
async fn write_and_register_roundtrip() -> anyhow::Result<()> {
    let rrd = bytes::Bytes::from(encode_rrd()?);

    let listener = re_grpc_server::ServerListener::bind((std::net::Ipv4Addr::LOCALHOST, 0).into())?;
    let (handler, write_upload_route) =
        RerunCloudHandlerBuilder::new().build_with_write_access()?;
    let handle = ServerBuilder::default()
        .with_listener(listener)
        .with_service(RerunCloudServiceServer::new(handler))
        .with_http_route("/upload/{grant}", write_upload_route)
        .build()
        .start(&re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?)
        .await?;

    let origin = format!("rerun+http://{}", handle.connect_addr()).parse()?;
    let connection = ConnectionRegistry::new_without_stored_credentials().connection_handle(origin);

    let storage_url = connection
        .write_object(ObjectKey::try_new("user/project/recording.rrd")?, rrd)
        .await?;

    let dataset_name = EntryName::new("uploads")?;
    let dataset_id = connection
        .client()
        .await?
        .find_or_create_dataset(&dataset_name)
        .await?;
    let segment_ids = connection
        .register_with_dataset(
            dataset_id,
            vec![DataSource::new_rrd_url(storage_url)],
            IfDuplicateBehavior::Overwrite,
        )
        .await?
        .wait(REGISTRATION_TIMEOUT)
        .await?;
    assert_eq!(segment_ids.len(), 1);

    handle.shutdown_and_wait().await;
    Ok(())
}

fn encode_rrd() -> anyhow::Result<Vec<u8>> {
    let store_id = StoreId::random(StoreKind::Recording, "write_grant_test");
    let points = MyPoint::from_iter(0..1);
    let chunk = Chunk::builder(EntityPath::from("/test/entity"))
        .with_sparse_component_batches(
            RowId::new(),
            TimePoint::default().with(Timeline::new_sequence("frame"), 0),
            [(MyPoints::descriptor_points(), Some(&points as _))],
        )
        .build()?;

    let mut bytes = Vec::new();
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_build_info::CrateVersion::LOCAL,
        re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED,
        &mut bytes,
    )?;
    encoder.append(&LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: *RowId::ZERO,
        info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
    }))?;
    encoder.append(&LogMsg::ArrowMsg(store_id, chunk.to_arrow_msg()?))?;
    encoder.finish()?;
    drop(encoder);
    Ok(bytes)
}
