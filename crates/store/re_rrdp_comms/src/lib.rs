//! Communications with an RRDP GRPC server.

#![allow(clippy::unwrap_used)] // TODO

use std::str::FromStr;

use anyhow::Context as _;
use re_chunk::Chunk;
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
};
use re_remote_store_types::{
    codec::decode,
    v0::{
        storage_node_client::StorageNodeClient, EncoderVersion, FetchRecordingRequest, RecordingId,
    },
};
use tokio_stream::StreamExt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidAddressError(#[from] InvalidAddressError),
}

#[derive(thiserror::Error, Debug)]
#[error("URL {url:?} should follow rrdp://addr:port/recording/12721")]
pub struct InvalidAddressError {
    url: String,
    msg: String,
}

type Result<T = (), E = Error> = std::result::Result<T, E>;

/// Parsed `rrdp://addr:port/recording/12721`
struct Address {
    addr_port: String,
    recording_id: String,
}

impl std::str::FromStr for Address {
    type Err = InvalidAddressError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let Some(stripped_url) = url.strip_prefix("rrdp://") else {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Missing rrdp://".to_owned(),
            });
        };

        let parts = stripped_url.split('/').collect::<Vec<_>>();
        if parts.len() < 3 {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Too few slashes".to_owned(),
            });
        }
        if parts.len() > 3 {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Too many slashes".to_owned(),
            });
        }

        if parts[1] != "recording" {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Not a recording".to_owned(),
            });
        }

        let addr_port = parts[0].to_owned();
        let recording_id = parts[2].to_owned();

        Ok(Self {
            addr_port,
            recording_id,
        })
    }
}

/// Stream an rrd file from an RRDP server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_recording(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>> {
    re_log::debug!("Loading {url}…");

    let address = Address::from_str(&url)?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdpStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RrdpStream { url: url.clone() },
    );

    tokio::spawn(async move {
        if let Err(err) = stream_recording_async(tx, address, on_msg).await {
            re_log::warn!(
                "Failed to fetch whole recording from {url}: {}",
                re_error::format(err)
            );
        }
    });

    Ok(rx)
}

async fn stream_recording_async(
    tx: re_smart_channel::Sender<LogMsg>,
    address: Address,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> anyhow::Result<()> {
    let Address {
        addr_port,
        recording_id,
    } = address;

    let http_addr = format!("http://{addr_port}");
    re_log::debug!("Connecting to {http_addr}…");

    let mut client = StorageNodeClient::connect(http_addr)
        .await?
        .max_decoding_message_size(1024 * 1024 * 1024);

    re_log::debug!("Fetching {recording_id}…");

    let mut resp = client
        .fetch_recording(FetchRecordingRequest {
            recording_id: Some(RecordingId {
                id: recording_id.clone(),
            }),
        })
        .await
        .context("fetch_recording failed")?
        .into_inner();

    // TODO(jleibs): Does this come from RDP?
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.clone());

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
    while let Some(batch) = resp.next().await {
        let raw = batch.unwrap();
        let tc = decode(EncoderVersion::V0, &raw.payload).unwrap().unwrap();

        let chunk = Chunk::from_transport(&tc).context("Bad Chunk")?;

        if tx
            .send(LogMsg::ArrowMsg(
                store_id.clone(),
                chunk.to_arrow_msg().context("to_arrow_msg")?,
            ))
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
