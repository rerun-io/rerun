//! Communications with an Rerun Data Platform gRPC server.

mod address;

use address::CatalogAddress;
pub use address::{InvalidRedapAddress, RecordingAddress};
use url::Url;

// ----------------------------------------------------------------------------

use std::error::Error;

use re_chunk::Chunk;
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
};
use re_protos::{
    codec::{decode, CodecError},
    v0::{
        storage_node_client::StorageNodeClient, FetchRecordingRequest, QueryCatalogRequest,
        RecordingId,
    },
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

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),
}

// ----------------------------------------------------------------------------

/// Stream recordings catalog over gRPC from Rerun Data Platform.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_catalog(
    redap_url: url::Url,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidRedapAddress> {
    re_log::debug!("Loading {redap_url}…");

    let address = redap_url.clone().try_into()?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream {
            url: redap_url.clone().to_string(),
        },
        re_smart_channel::SmartChannelSource::RerunGrpcStream {
            url: redap_url.clone().to_string(),
        },
    );

    spawn_future(async move {
        if let Err(err) = stream_catalog_async(tx, address, on_msg).await {
            re_log::warn!(
                "Error while streaming {redap_url}: {}",
                re_error::format_ref(&err)
            );
        }
    });

    Ok(rx)
}

/// Stream an rrd file over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_recording(
    redap_url: Url,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidRedapAddress> {
    re_log::debug!("Loading {redap_url}…");

    let address = redap_url.clone().try_into()?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream {
            url: redap_url.clone().to_string(),
        },
        re_smart_channel::SmartChannelSource::RerunGrpcStream {
            url: redap_url.clone().to_string(),
        },
    );

    spawn_future(async move {
        if let Err(err) = stream_recording_async(tx, address, on_msg).await {
            re_log::warn!(
                "Error while streaming {redap_url}: {}",
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
    address: RecordingAddress,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    let RecordingAddress {
        redap_endpoint,
        recording_id,
    } = address;

    let mut client = connect(redap_endpoint).await?;
    re_log::debug!("Fetching {recording_id}…");

    let mut resp = client
        .fetch_recording(FetchRecordingRequest {
            recording_id: Some(RecordingId {
                id: recording_id.clone(),
            }),
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .filter_map(|resp| {
            resp.and_then(|r| {
                decode(r.encoder_version(), &r.payload)
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
            .transpose()
        });

    drop(client);

    // TODO(zehiko) - we need a separate gRPC endpoint for fetching Store info REDAP #85
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
        let tc = result.map_err(TonicStatusError)?;
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

/// TODO(zehiko) - this is a copy of `stream_recording_async` with a different gRPC call,
/// this will go away as we tackle unification of data and metadata streams REDAP #74, hence
/// avoiding refactoring right now
async fn stream_catalog_async(
    tx: re_smart_channel::Sender<LogMsg>,
    address: CatalogAddress,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    let mut client = connect(address.redap_endpoint).await?;
    re_log::debug!("Fetching catalog…");

    let mut resp = client
        // TODO(zehiko) add support for fetching specific columns and rows
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: None,            // fetch all rows
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .filter_map(|resp| {
            resp.and_then(|r| {
                decode(r.encoder_version(), &r.payload)
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
            .transpose()
        });

    drop(client);

    // We need a whole StoreInfo here.
    let store_id = StoreId::from_string(StoreKind::Recording, "catalog".to_owned());

    let store_info = StoreInfo {
        application_id: ApplicationId::from("rerun_data_platform"),
        store_id: store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::Unknown,
        store_version: None,
    };

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
        let tc = result.map_err(TonicStatusError)?;
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

/// Connect to a Rerun Data Platform gRPC server. We use different gRPC clients depending on the
/// platform (web vs native).
async fn connect(
    redap_endpoint: Url,
) -> Result<StorageNodeClient<tonic::transport::Channel>, StreamError> {
    re_log::debug!("Connecting to {redap_endpoint}…");

    let client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            http_addr,
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

    Ok(client)
}
