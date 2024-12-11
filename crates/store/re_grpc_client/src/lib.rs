//! Communications with an Rerun Data Platform gRPC server.

mod address;

pub use address::{InvalidRedapAddress, RedapAddress};
use re_chunk::external::arrow2;
use re_log_types::external::re_types_core::ComponentDescriptor;
use re_types::components::RecordingUri;
use re_types::Component;
use url::Url;

// ----------------------------------------------------------------------------

use std::error::Error;

use arrow2::array::Utf8Array as Arrow2Utf8Array;
use arrow2::datatypes::Field as Arrow2Field;
use re_chunk::{Arrow2Array, Chunk};
use re_log_encoding::codec::{wire::decode, CodecError};
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
};
use re_protos::common::v0::RecordingId;
use re_protos::remote_store::v0::{
    storage_node_client::StorageNodeClient, FetchRecordingRequest, QueryCatalogRequest,
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

    #[error("Invalid URI: {0}")]
    InvalidUri(String),
}

// ----------------------------------------------------------------------------

/// Stream an rrd file or metadsasta catalog over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_from_redap(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidRedapAddress> {
    re_log::debug!("Loading {url}…");

    let address = url.as_str().try_into()?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RerunGrpcStream { url: url.clone() },
    );

    spawn_future(async move {
        match address {
            RedapAddress::Recording {
                redap_endpoint,
                recording_id,
            } => {
                if let Err(err) =
                    stream_recording_async(tx, redap_endpoint, recording_id, on_msg).await
                {
                    re_log::warn!(
                        "Error while streaming {url}: {}",
                        re_error::format_ref(&err)
                    );
                }
            }
            RedapAddress::Catalog { redap_endpoint } => {
                if let Err(err) = stream_catalog_async(tx, redap_endpoint, on_msg).await {
                    re_log::warn!(
                        "Error while streaming {url}: {}",
                        re_error::format_ref(&err)
                    );
                }
            }
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
    redap_endpoint: Url,
    recording_id: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    re_log::debug!("Connecting to {redap_endpoint}…");
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

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
        application_id: ApplicationId::from("redap_recording"),
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

async fn stream_catalog_async(
    tx: re_smart_channel::Sender<LogMsg>,
    redap_endpoint: Url,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    re_log::debug!("Connecting to {redap_endpoint}…");
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

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
        application_id: ApplicationId::from("redap_catalog"),
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
        let mut chunk = Chunk::from_transport(&tc)?;

        // enrich catalog data with RecordingUri that's based on the ReDap endpoint (that we know)
        // and the recording id (that we have in the catalog data)
        let host = redap_endpoint
            .host()
            .ok_or(StreamError::InvalidUri(format!(
                "couldn't get host from {redap_endpoint}"
            )))?;
        let port = redap_endpoint
            .port()
            .ok_or(StreamError::InvalidUri(format!(
                "couldn't get port from {redap_endpoint}"
            )))?;

        let recording_uri_arrays: Vec<Box<dyn Arrow2Array>> = chunk
            .iter_component_arrays(&"id".into())
            .map(|id| {
                let rec_id = id
                    .as_any()
                    .downcast_ref::<Arrow2Utf8Array<i32>>()
                    .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
                        reason: format!("id must be a utf8 array: {:?}", tc.schema),
                    }))?
                    .value(0); // each component batch is of length 1 i.e. single 'id' value

                let recording_uri = format!("rerun://{host}:{port}/recording/{rec_id}");

                let recording_uri_data = Arrow2Utf8Array::<i32>::from([Some(recording_uri)]);

                Ok::<Box<_>, StreamError>(
                    Box::new(recording_uri_data) as Box<dyn arrow2::array::Array>
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let recording_id_arrays = recording_uri_arrays
            .iter()
            .map(|e| Some(e.as_ref()))
            .collect::<Vec<_>>();

        let rec_id_field = Arrow2Field::new("item", arrow2::datatypes::DataType::Utf8, true);
        #[allow(clippy::unwrap_used)] // we know we've given the right field type
        let uris = re_chunk::util::arrays_to_list_array(
            rec_id_field.data_type().clone(),
            &recording_id_arrays,
        )
        .unwrap();

        chunk.add_component(ComponentDescriptor::new(RecordingUri::name()), uris)?;

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
