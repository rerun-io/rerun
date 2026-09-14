//! Client-side write grants against a running `re_server`.
//!
//! Exercises the intended API end to end: acquire a grant, redeem it by uploading an in-memory
//! RRD, then register the credential-free storage URL with a dataset.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{
    EntityPath, EntryName, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_protos::cloud::v1alpha1::access_grant::Redemption;
use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;
use re_protos::cloud::v1alpha1::{AccessGrant, GetWriteAccessGrantRequest};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionRegistry;
use re_server::{RerunCloudHandlerBuilder, ServerBuilder};

const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test(flavor = "multi_thread")]
#[ignore = "TODO(grtlr): `GetWriteAccessGrant` is not implemented yet"]
async fn write_and_register_roundtrip() -> anyhow::Result<()> {
    let rrd = bytes::Bytes::from(encode_rrd()?);

    let handle = ServerBuilder::default()
        .with_address((std::net::Ipv4Addr::LOCALHOST, 0).into())
        .with_service(RerunCloudServiceServer::new(
            RerunCloudHandlerBuilder::new().build(),
        ))
        .build()
        .start(&re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?)
        .await?;

    let origin = format!("rerun+http://{}", handle.connect_addr()).parse()?;
    let connection = ConnectionRegistry::new_without_stored_credentials().connection_handle(origin);

    let mut client = connection.client().await?;
    let response = client
        .inner()
        .get_write_access_grant(GetWriteAccessGrantRequest {
            size_bytes: rrd.len().try_into()?,
            key: "user/project/recording.rrd".to_owned(),
            location: None,
        })
        .await?
        .into_inner();

    let storage_url = response.storage_url.parse()?;
    let AccessGrant {
        expires_at,
        redemption,
    } = response
        .grant
        .ok_or_else(|| anyhow::anyhow!("write access grant is missing"))?;
    let _expires_at = expires_at.ok_or_else(|| anyhow::anyhow!("grant expiry is missing"))?;
    let Redemption::HttpRequest(http_request) =
        redemption.ok_or_else(|| anyhow::anyhow!("grant redemption is missing"))?;

    let upload_response = ehttp::fetch_async(ehttp::Request {
        method: ehttp::Method::parse(&http_request.method).map_err(anyhow::Error::msg)?,
        url: http_request.url,
        body: rrd.to_vec(),
        headers: ehttp::Headers {
            headers: http_request
                .headers
                .into_iter()
                .map(|header| (header.name, header.value))
                .collect(),
        },
        timeout: Some(Duration::from_secs(30)),
    })
    .await
    .map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        upload_response.ok,
        "upload failed: HTTP {} {}",
        upload_response.status,
        upload_response.status_text
    );

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
