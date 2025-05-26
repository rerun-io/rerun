use tokio_stream::{Stream, StreamExt as _};

use re_auth::client::AuthDecorator;
use re_chunk::Chunk;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::catalog::v1alpha1::ReadDatasetEntryRequest;
use re_protos::frontend::v1alpha1::frontend_service_client::FrontendServiceClient;
use re_protos::{
    catalog::v1alpha1::ext::ReadDatasetEntryResponse, frontend::v1alpha1::GetChunksRequest,
};
use re_uri::{DatasetDataUri, Origin};

use crate::{MAX_DECODING_MESSAGE_SIZE, StreamError, spawn_future};

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
pub fn stream_dataset_from_redap(
    uri: DatasetDataUri,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    re_log::debug!("Loading {uri}…");

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RedapGrpcStream { uri: uri.clone() },
        re_smart_channel::SmartChannelSource::RedapGrpcStream { uri: uri.clone() },
    );

    spawn_future(async move {
        if let Err(err) = stream_partition_async(tx, uri.clone(), on_cmd, on_msg).await {
            re_log::error!(
                "Error while streaming {uri}: {}",
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
pub type RedapClient = FrontendServiceClient<
    tonic::service::interceptor::InterceptedService<tonic_web_wasm_client::Client, AuthDecorator>,
>;

#[cfg(target_arch = "wasm32")]
pub async fn client(origin: Origin) -> Result<RedapClient, ConnectionError> {
    let channel = channel(origin).await?;

    //TODO(ab): actually provide a token here
    let auth = AuthDecorator::new(None);

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::interceptor(auth))
        // TODO(cmc): figure out how we integrate redap_telemetry in mainline Rerun
        // .layer(redap_telemetry::new_grpc_tracing_layer())
        // .layer(redap_telemetry::TracingInjectorInterceptor::new_layer())
        .into_inner();

    let svc = tower::ServiceBuilder::new()
        .layer(middlewares)
        .service(channel);

    Ok(FrontendServiceClient::new(svc).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(not(target_arch = "wasm32"))]
pub type RedapClient = FrontendServiceClient<
    tonic::service::interceptor::InterceptedService<tonic::transport::Channel, AuthDecorator>,
>;
// TODO(cmc): figure out how we integrate redap_telemetry in mainline Rerun
// pub type RedapClient = FrontendServiceClient<
//     tower_http::trace::Trace<
//         tonic::service::interceptor::InterceptedService<
//             tonic::transport::Channel,
//             redap_telemetry::TracingInjectorInterceptor,
//         >,
//         tower_http::classify::SharedClassifier<tower_http::classify::GrpcErrorsAsFailures>,
//     >,
// >;

#[cfg(not(target_arch = "wasm32"))]
pub async fn client(origin: Origin) -> Result<RedapClient, ConnectionError> {
    let channel = channel(origin).await?;

    // Use the token from the env var if available, logging any error
    //
    // TODO(rerun-io/dataplatform#721): we should support configuring the token from
    // the UI and/or API too
    let maybe_token = std::env::var("REDAP_TOKEN")
        .map_err(|err| match err {
            std::env::VarError::NotPresent => {}
            std::env::VarError::NotUnicode(..) => {
                re_log::warn_once!("REDAP_TOKEN env var is malformed: {err}");
            }
        })
        .and_then(|t| {
            re_auth::Jwt::try_from(t).map_err(|err| {
                re_log::warn_once!(
                    "REDAP_TOKEN env var is present, but the token is invalid: {err}"
                );
            })
        })
        .ok();

    let auth = AuthDecorator::new(maybe_token);

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::interceptor(auth))
        // TODO(cmc): figure out how we integrate redap_telemetry in mainline Rerun
        // .layer(redap_telemetry::new_grpc_tracing_layer())
        // .layer(redap_telemetry::TracingInjectorInterceptor::new_layer())
        .into_inner();

    let svc = tower::ServiceBuilder::new()
        .layer(middlewares)
        .service(channel);

    Ok(FrontendServiceClient::new(svc).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

/// Converts a `FetchPartitionResponse` stream into a stream of `Chunk`s.
//TODO(#9430): ideally this should be factored as a nice helper in `re_proto`
//TODO(#9497): This is a hack to extract the partition id from the record batch before they are lost to
//the `Chunk` conversion. The chunks should instead include that information.
pub fn get_chunks_response_to_chunk_and_partition_id(
    response: tonic::Streaming<re_protos::manifest_registry::v1alpha1::GetChunksResponse>,
) -> impl Stream<Item = Result<(Chunk, Option<String>), StreamError>> {
    response.map(|resp| {
        resp.map_err(Into::into).and_then(|r| {
            let batch = r.chunk.ok_or(StreamError::MissingChunkData)?.decode()?;

            let partition_id = batch.schema().metadata().get("rerun.partition_id").cloned();
            let chunk = Chunk::from_record_batch(&batch).map_err(Into::<StreamError>::into)?;

            Ok((chunk, partition_id))
        })
    })
}

pub async fn stream_partition_async(
    tx: re_smart_channel::Sender<LogMsg>,
    uri: re_uri::DatasetDataUri,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    let re_uri::DatasetDataUri {
        origin,
        dataset_id,
        partition_id,
        time_range,
        fragment: _, // Only affects the viewer
    } = uri;

    re_log::debug!("Connecting to {}…", origin);
    let mut client = client(origin).await?;

    re_log::debug!("Fetching catalog data for partition {partition_id} of dataset {dataset_id}…");

    let read_dataset_response: ReadDatasetEntryResponse = client
        .read_dataset_entry(ReadDatasetEntryRequest {
            id: Some(dataset_id.into()),
        })
        .await?
        .into_inner()
        .try_into()?;

    let dataset_name = read_dataset_response.dataset_entry.details.name;

    let catalog_chunk_stream = client
        .get_chunks(GetChunksRequest {
            dataset_id: Some(dataset_id.into()),
            partition_ids: vec![partition_id.clone().into()],
            chunk_ids: vec![],
            entity_paths: vec![],
            query: None,
        })
        .await?
        .into_inner();

    drop(client);

    let store_id = StoreId::from_string(StoreKind::Recording, partition_id.clone());
    let store_info = StoreInfo {
        application_id: dataset_name.into(),
        store_id: store_id.clone(),
        cloned_from: None,
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    if let Some(time_range) = time_range {
        on_cmd(Command::SetLoopSelection {
            recording_id: store_id.clone(),
            timeline: time_range.timeline,
            time_range: time_range.into(),
        });
    }

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

    let mut chunk_stream = get_chunks_response_to_chunk_and_partition_id(catalog_chunk_stream);

    while let Some(chunk) = chunk_stream.next().await {
        let (chunk, _partition_id) = chunk?;

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
