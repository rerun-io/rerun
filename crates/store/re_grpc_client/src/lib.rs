//! Communications with an Rerun Data Platform gRPC server.

mod address;

pub use address::{Address, InvalidAddressError};

// ----------------------------------------------------------------------------

use std::{error::Error, str::FromStr};

use re_chunk::Chunk;
use re_log_encoding::codec::{decode, CodecError};
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
};
use re_protos::{
    common::v0::{EncoderVersion, RecordingId},
    remote_store::v0::{storage_node_client::StorageNodeClient, FetchRecordingRequest},
};

// ----------------------------------------------------------------------------

/// Wrapper with a nicer error message
#[derive(Debug)]
struct TonicStatusError(tonic::Status);

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = &self.0;
        write!(f, "gRPC error, status: '{}'", status.code())?;
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata())?;
        }
        Ok(())
    }
}

impl Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

#[derive(thiserror::Error, Debug)]
enum StreamError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),

    #[error(transparent)]
    TonicStatus(#[from] TonicStatusError),

    #[error("Missing TransportChunk")]
    MissingTransportChunk,

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),
}

// ----------------------------------------------------------------------------

/// Stream an rrd file over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_recording(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidAddressError> {
    re_log::debug!("Loading {url}…");

    let address = Address::from_str(&url)?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RerunGrpcStream { url: url.clone() },
    );

    spawn_future(async move {
        if let Err(err) = stream_recording_async(tx, address, on_msg).await {
            re_log::warn!(
                "Error while streaming {url}: {}",
                re_error::format_ref(&err)
            );
        }
    });

    Ok(rx)
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}

async fn stream_recording_async(
    tx: re_smart_channel::Sender<LogMsg>,
    address: Address,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    let Address {
        addr_port,
        recording_id,
    } = address;

    if addr_port.starts_with("0.0.0.0:") {
        re_log::warn!("Attempting to connect to IP 0.0.0.0. This will often fail. You likely you want connect to 127.0.0.1 instead.");
    }

    let http_addr = format!("http://{addr_port}");
    re_log::debug!("Connecting to {http_addr}…");

    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            http_addr,
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(http_addr)?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

    client = client.max_decoding_message_size(1024 * 1024 * 1024);

    re_log::debug!("Fetching {recording_id}…");

    let mut resp = client
        .fetch_recording(FetchRecordingRequest {
            recording_id: Some(RecordingId {
                id: recording_id.clone(),
            }),
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner();

    drop(client);

    // TODO(jleibs): Does this come from RDP?
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.clone());

    let store_info = StoreInfo {
        application_id: ApplicationId::from("rerun_data_platform"),
        store_id: store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    // We need a whole StoreInfo here.
    if tx
        .send(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *re_chunk::RowId::new(),
            info: store_info,
        }))
        .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(());
    }

    re_log::info!("Starting to read...");
    while let Some(result) = resp.next().await {
        let response = result.map_err(TonicStatusError)?;
        let tc = decode(EncoderVersion::V0, &response.payload)?;

        let Some(tc) = tc else {
            return Err(StreamError::MissingTransportChunk);
        };

        let chunk = Chunk::from_transport(&tc)?;

        if tx
            .send(LogMsg::ArrowMsg(store_id.clone(), chunk.to_arrow_msg()?))
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }

        if let Some(on_msg) = &on_msg {
            on_msg();
        }
    }

    Ok(())
}
