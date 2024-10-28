//! Communications with an RRDP GRPC server.

use re_chunk::{Chunk, RowId};
use re_log_types::{
    ApplicationId, FileSource, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
    Time,
};
use re_remote_store_types::{
    codec::decode,
    v0::{
        storage_node_client::StorageNodeClient, EncoderVersion, FetchRecordingRequest, RecordingId,
    },
};
use tokio_stream::StreamExt;

/// Stream an rrd file from an RRDP server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_recording(
    url: String,
    _on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    // TODO(jleibs): Where should we actually be creating this
    let rt = tokio::runtime::Runtime::new().unwrap();

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdpStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RrdpStream { url: url.clone() },
    );

    rt.spawn(stream_recording_async(tx, url));

    std::mem::forget(rt);

    rx
}

async fn stream_recording_async(tx: re_smart_channel::Sender<LogMsg>, url: String) {
    // Extract addr and recording_id from the url
    // rdp://addr:port/recording/12721

    let stripped_url = url.strip_prefix("rrdp://").unwrap().to_owned();

    let parts = stripped_url
        .split('/')
        .take(3)
        .map(|p| p.to_string())
        .collect::<Vec<_>>();
    let addr = &parts[0];
    let recording_id = &parts[2];

    let addr = format!("http://{}", addr);

    re_log::info!("Connecting to {}", addr);

    let mut client = StorageNodeClient::connect(addr).await.unwrap();
    let mut client = client.max_decoding_message_size(1024 * 1024 * 1024);

    let mut resp = client
        .fetch_recording(FetchRecordingRequest {
            recording_id: Some(RecordingId {
                id: recording_id.to_owned(),
            }),
        })
        .await
        .unwrap()
        .into_inner();

    // TODO(jleibs): Does this come from RDP?
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.to_owned());

    let store_info = StoreInfo {
        application_id: ApplicationId::from("rrdp"),
        store_id: store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    // We need a whole StoreInfo here.
    tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: *re_chunk::RowId::new(),
        info: store_info,
    }));

    re_log::info!("Starting to read...");
    while let Some(batch) = resp.next().await {
        let raw = batch.unwrap();
        let tc = decode(EncoderVersion::V0, &raw.payload).unwrap().unwrap();

        let chunk = Chunk::from_transport(&tc).unwrap();

        tx.send(LogMsg::ArrowMsg(
            store_id.clone(),
            chunk.to_arrow_msg().unwrap(),
        ))
        .unwrap();
    }
}
