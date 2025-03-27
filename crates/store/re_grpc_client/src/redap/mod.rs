use arrow::array::RecordBatch as ArrowRecordBatch;

use tokio_stream::StreamExt as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::Chunk;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_protos::frontend::v1alpha1::frontend_service_client::FrontendServiceClient;
use re_protos::remote_store::v1alpha1::{
    storage_node_service_client::StorageNodeServiceClient, CatalogEntry, GetChunksRangeRequest,
};
use re_protos::{
    common::v1alpha1::{IndexColumnSelector, RecordingId},
    remote_store::v1alpha1::{
        CatalogFilter, FetchRecordingRequest, QueryCatalogRequest, CATALOG_APP_ID_FIELD_NAME,
    },
};
use re_uri::{Origin, RecordingEndpoint};

use crate::{spawn_future, StreamError, MAX_DECODING_MESSAGE_SIZE};

pub enum Command {
    SetLoopSelection {
        recording_id: re_log_types::StoreId,
        timeline: re_log_types::Timeline,
        time_range: re_log_types::ResolvedTimeRangeF,
    },
}

/// Stream an rrd file or metadata catalog over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_from_redap(
    endpoint: RecordingEndpoint,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    re_log::debug!("Loading {endpoint}…");

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RedapGrpcStream(endpoint.clone()),
        re_smart_channel::SmartChannelSource::RedapGrpcStream(endpoint.clone()),
    );

    spawn_future(async move {
        if let Err(err) = stream_recording_async(tx, endpoint.clone(), on_cmd, on_msg).await {
            re_log::error!(
                "Error while streaming {endpoint}: {}",
                re_error::format_ref(&err)
            );
        }
    });

    rx
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Connection error: {0}")]
    Tonic(#[from] tonic::transport::Error),

    #[error("server is expecting an unencrypted connection (try `rerun+http://` if you are sure)")]
    UnencryptedServer,

    #[error("invalid origin: {0}")]
    InvalidOrigin(String),
}

#[cfg(target_arch = "wasm32")]
pub async fn channel(origin: Origin) -> Result<tonic_web_wasm_client::Client, ConnectionError> {
    let channel = tonic_web_wasm_client::Client::new_with_options(
        origin.as_url(),
        tonic_web_wasm_client::options::FetchOptions::new(),
    );

    Ok(channel)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn channel(origin: Origin) -> Result<tonic::transport::Channel, ConnectionError> {
    use std::net::Ipv4Addr;

    use tonic::transport::Endpoint;

    let http_url = origin.as_url();

    match Endpoint::new(http_url)?
        .tls_config(
            tonic::transport::ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true),
        )?
        .connect()
        .await
    {
        Ok(channel) => Ok(channel),
        Err(original_error) => {
            if ![
                url::Host::Domain("localhost".to_owned()),
                url::Host::Ipv4(Ipv4Addr::new(127, 0, 0, 1)),
            ]
            .contains(&origin.host)
            {
                return Err(ConnectionError::Tonic(original_error));
            }

            // If we can't establish a connection, we probe if the server is
            // expecting unencrypted traffic. If that is the case, we return
            // a more meaningful error message.
            let Ok(endpoint) = Endpoint::new(origin.coerce_http_url()) else {
                return Err(ConnectionError::Tonic(original_error));
            };

            if endpoint.connect().await.is_ok() {
                Err(ConnectionError::UnencryptedServer)
            } else {
                Err(ConnectionError::Tonic(original_error))
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn legacy_client(
    origin: Origin,
) -> Result<StorageNodeServiceClient<tonic_web_wasm_client::Client>, ConnectionError> {
    let channel = channel(origin).await?;
    Ok(StorageNodeServiceClient::new(channel).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn legacy_client(
    origin: Origin,
) -> Result<StorageNodeServiceClient<tonic::transport::Channel>, ConnectionError> {
    let channel = channel(origin).await?;
    Ok(StorageNodeServiceClient::new(channel).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn legacy_client_with_interceptor<I: tonic::service::Interceptor>(
    origin: Origin,
    interceptor: I,
) -> Result<
    StorageNodeServiceClient<
        tonic::service::interceptor::InterceptedService<tonic::transport::Channel, I>,
    >,
    ConnectionError,
> {
    let channel = channel(origin).await?;
    Ok(
        StorageNodeServiceClient::with_interceptor(channel, interceptor)
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE),
    )
}

#[cfg(target_arch = "wasm32")]
pub async fn client(
    origin: Origin,
) -> Result<FrontendServiceClient<tonic_web_wasm_client::Client>, ConnectionError> {
    let channel = channel(origin).await?;
    Ok(FrontendServiceClient::new(channel).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn client(
    origin: Origin,
) -> Result<FrontendServiceClient<tonic::transport::Channel>, ConnectionError> {
    let channel = channel(origin).await?;
    Ok(FrontendServiceClient::new(channel).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn client_with_interceptor<I: tonic::service::Interceptor>(
    origin: Origin,
    interceptor: I,
) -> Result<
    FrontendServiceClient<
        tonic::service::interceptor::InterceptedService<tonic::transport::Channel, I>,
    >,
    ConnectionError,
> {
    let channel = channel(origin).await?;
    Ok(
        FrontendServiceClient::with_interceptor(channel, interceptor)
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE),
    )
}

pub async fn stream_recording_async(
    tx: re_smart_channel::Sender<LogMsg>,
    endpoint: re_uri::RecordingEndpoint,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    re_log::debug!("Connecting to {}…", endpoint.origin);
    let mut client = legacy_client(endpoint.origin).await?;

    re_log::debug!("Fetching catalog data for {}…", endpoint.recording_id);

    let catalog_chunk_stream = client
        .query_catalog(QueryCatalogRequest {
            entry: Some(CatalogEntry {
                name: "default".to_owned(), /* TODO(zehiko) 9116 */
            }),
            column_projection: None, // fetch all columns
            filter: Some(CatalogFilter {
                recording_ids: vec![RecordingId {
                    id: endpoint.recording_id.clone(),
                }],
            }),
        })
        .await?
        .into_inner();

    let catalog_chunks = catalog_chunk_stream
        .map(|resp| {
            resp.and_then(|r| {
                r.data
                    .ok_or_else(|| {
                        tonic::Status::internal("missing DataframePart in QueryCatalogResponse")
                    })?
                    .decode()
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
        })
        .collect::<Result<Vec<_>, tonic::Status>>()
        .await?;

    if catalog_chunks.len() != 1 || catalog_chunks[0].num_rows() != 1 {
        return Err(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!(
                "expected exactly one recording with id {}, got {}",
                endpoint.recording_id,
                catalog_chunks.len()
            ),
        }));
    }

    let store_info =
        store_info_from_catalog_chunk(&catalog_chunks[0].clone(), &endpoint.recording_id)?;
    let store_id = store_info.store_id.clone();

    re_log::debug!("Fetching {}…", endpoint.recording_id);

    let mut chunk_stream = if let Some(time_range) = &endpoint.time_range {
        let stream = client
            .get_chunks_range(GetChunksRangeRequest {
                entry: Some(CatalogEntry {
                    name: "default".to_owned(), /* TODO(zehiko) 9116 */
                }),
                recording_id: Some(RecordingId {
                    id: endpoint.recording_id.clone(),
                }),
                time_index: Some(IndexColumnSelector {
                    timeline: Some(time_range.timeline.into()),
                }),
                time_range: Some(
                    re_log_types::ResolvedTimeRange::new(
                        // min.floor()..min.ceil() should cover the entire requested range
                        time_range.range.min.floor(),
                        time_range.range.max.ceil(),
                    )
                    .into(),
                ),
            })
            .await?
            .into_inner()
            .map(|res| res.map(|v| v.chunk));
        futures::future::Either::Left(stream)
    } else {
        let stream = client
            .fetch_recording(FetchRecordingRequest {
                entry: Some(CatalogEntry {
                    name: "default".to_owned(), /* TODO(zehiko) 9116 */
                }),
                recording_id: Some(RecordingId {
                    id: endpoint.recording_id.clone(),
                }),
            })
            .await?
            .into_inner()
            .map(|res| res.map(|v| v.chunk));
        futures::future::Either::Right(stream)
    }
    .map(|resp| {
        resp.and_then(|r| {
            r.ok_or_else(|| tonic::Status::internal("missing Chunk in Response"))?
                .decode()
                .map_err(|err| tonic::Status::internal(err.to_string()))
        })
    });

    drop(client);

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

    if let Some(time_range) = endpoint.time_range {
        on_cmd(Command::SetLoopSelection {
            recording_id: StoreId::from_string(StoreKind::Recording, endpoint.recording_id),
            timeline: time_range.timeline,
            time_range: time_range.range,
        });
    }

    while let Some(result) = chunk_stream.next().await {
        let batch = result?;
        let chunk = Chunk::from_record_batch(&batch)?;

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

pub fn store_info_from_catalog_chunk(
    record_batch: &ArrowRecordBatch,
    recording_id: &str,
) -> Result<StoreInfo, StreamError> {
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.to_owned());

    let data = record_batch
        .column_by_name(CATALOG_APP_ID_FIELD_NAME)
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!("no {CATALOG_APP_ID_FIELD_NAME} field found"),
        }))?;
    let app_id = data
        .downcast_array_ref::<arrow::array::StringArray>()
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!(
                "{CATALOG_APP_ID_FIELD_NAME} must be a utf8 array: {:?}",
                record_batch.schema_ref()
            ),
        }))?
        .value(0);

    Ok(StoreInfo {
        application_id: ApplicationId::from(app_id),
        store_id: store_id.clone(),
        cloned_from: None,
        store_source: StoreSource::Unknown,
        store_version: None,
    })
}
