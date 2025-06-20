use tokio_stream::{Stream, StreamExt as _};

use re_auth::client::AuthDecorator;
use re_chunk::Chunk;
use re_log_types::{
    BlueprintActivationCommand, EntryId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind,
    StoreSource,
};
use re_protos::catalog::v1alpha1::ReadDatasetEntryRequest;
use re_protos::common::v1alpha1::ext::PartitionId;
use re_protos::frontend::v1alpha1::frontend_service_client::FrontendServiceClient;
use re_protos::{
    catalog::v1alpha1::ext::ReadDatasetEntryResponse, frontend::v1alpha1::GetChunksRequest,
};
use re_uri::{DatasetDataUri, Origin, TimeRange};

use crate::{
    ConnectionClient, ConnectionRegistryHandle, MAX_DECODING_MESSAGE_SIZE, StreamError,
    spawn_future,
};

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
    connection_registry: &ConnectionRegistryHandle,
    uri: DatasetDataUri,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    re_log::debug!("Loading {uri}…");

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RedapGrpcStream {
            uri: uri.clone(),
            select_when_loaded: true,
        },
        re_smart_channel::SmartChannelSource::RedapGrpcStream {
            uri: uri.clone(),
            select_when_loaded: true,
        },
    );

    async fn stream_partition(
        connection_registry: ConnectionRegistryHandle,
        tx: re_smart_channel::Sender<LogMsg>,
        uri: DatasetDataUri,
        on_cmd: Box<dyn Fn(Command) + Send + Sync>,
        on_msg: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> Result<(), StreamError> {
        let client = connection_registry.client(uri.origin.clone()).await?;

        stream_blueprint_and_partition_from_server(client, tx, uri.clone(), on_cmd, on_msg).await
    }

    let connection_registry = connection_registry.clone();
    spawn_future(async move {
        if let Err(err) =
            stream_partition(connection_registry, tx, uri.clone(), on_cmd, on_msg).await
        {
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

    let endpoint = {
        let mut endpoint = Endpoint::new(http_url)?.tls_config(
            tonic::transport::ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true),
        )?;

        if false {
            // NOTE: Tried it, had no noticeable effects in any of my benchmarks.
            endpoint = endpoint.initial_stream_window_size(Some(4 * 1024 * 1024));
            endpoint = endpoint.initial_connection_window_size(Some(16 * 1024 * 1024));
        }

        endpoint.connect().await
    };

    match endpoint {
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
pub type RedapClientInner =
    tonic::service::interceptor::InterceptedService<tonic_web_wasm_client::Client, AuthDecorator>;

#[cfg(target_arch = "wasm32")]
pub(crate) async fn client(
    origin: Origin,
    token: Option<re_auth::Jwt>,
) -> Result<RedapClient, ConnectionError> {
    let channel = channel(origin).await?;

    let auth = AuthDecorator::new(token);

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::InterceptorLayer::new(auth))
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
pub type RedapClientInner =
    tonic::service::interceptor::InterceptedService<tonic::transport::Channel, AuthDecorator>;

// TODO(cmc): figure out how we integrate redap_telemetry in mainline Rerun
// pub type RedapClientInner = tower_http::trace::Trace<
//     tonic::service::interceptor::InterceptedService<
//         tonic::transport::Channel,
//         redap_telemetry::TracingInjectorInterceptor,
//     >,
//     tower_http::classify::SharedClassifier<tower_http::classify::GrpcErrorsAsFailures>,
// >;

pub type RedapClient = FrontendServiceClient<RedapClientInner>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn client(
    origin: Origin,
    token: Option<re_auth::Jwt>,
) -> Result<RedapClient, ConnectionError> {
    let channel = channel(origin).await?;

    let auth = AuthDecorator::new(token);

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::InterceptorLayer::new(auth))
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
//
// TODO(#9430): ideally this should be factored as a nice helper in `re_proto`
// TODO(cmc): we should compute contiguous runs of the same partition here, and return a `(String, Vec<Chunk>)`
// instead. Because of how the server performs the computation, this will very likely work out well
// in practice.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_chunks_response_to_chunk_and_partition_id(
    response: tonic::Streaming<re_protos::manifest_registry::v1alpha1::GetChunksResponse>,
) -> impl Stream<Item = Result<Vec<(Chunk, Option<String>)>, StreamError>> {
    response
        .then(|resp| {
            // We want to make sure to offload that compute-heavy work to the compute worker pool: it's
            // not going to make this one single pipeline any faster, but it will prevent starvation of
            // the Tokio runtime (which would slow down every other futures currently scheduled!).
            tokio::task::spawn_blocking(move || {
                resp.map_err(Into::<StreamError>::into).and_then(|r| {
                    let _span = tracing::trace_span!(
                        "get_chunks::batch_decode",
                        num_chunks = r.chunks.len()
                    )
                    .entered();

                    r.chunks
                        .into_iter()
                        .map(|arrow_msg| {
                            let partition_id = arrow_msg.store_id.clone().map(|id| id.id);

                            let arrow_msg =
                                re_log_encoding::protobuf_conversions::arrow_msg_from_proto(
                                    &arrow_msg,
                                )
                                .map_err(Into::<StreamError>::into)?;

                            let chunk = re_chunk::Chunk::from_record_batch(&arrow_msg.batch)
                                .map_err(Into::<StreamError>::into)?;

                            Ok((chunk, partition_id))
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
            })
        })
        .map(|res| {
            res.map_err(Into::<StreamError>::into)
                .and_then(std::convert::identity)
        })
}

// This code path happens to be shared between native and web, but we don't have a Tokio runtime on web!
#[cfg(target_arch = "wasm32")]
pub fn get_chunks_response_to_chunk_and_partition_id(
    response: tonic::Streaming<re_protos::manifest_registry::v1alpha1::GetChunksResponse>,
) -> impl Stream<Item = Result<Vec<(Chunk, Option<String>)>, StreamError>> {
    response.map(|resp| {
        resp.map_err(Into::into).and_then(|r| {
            let _span =
                tracing::trace_span!("get_chunks::batch_decode", num_chunks = r.chunks.len())
                    .entered();

            r.chunks
                .into_iter()
                .map(|arrow_msg| {
                    let partition_id = arrow_msg.store_id.clone().map(|id| id.id);

                    let arrow_msg =
                        re_log_encoding::protobuf_conversions::arrow_msg_from_proto(&arrow_msg)
                            .map_err(Into::<StreamError>::into)?;

                    let chunk = re_chunk::Chunk::from_record_batch(&arrow_msg.batch)
                        .map_err(Into::<StreamError>::into)?;

                    Ok((chunk, partition_id))
                })
                .collect::<Result<Vec<_>, _>>()
        })
    })
}

/// Canonical way to ingest partition data from a Rerun data platform server, dealing with
/// server-stored blueprints if any.
///
/// The current strategy currently consists of _always_ downloading the blueprint first and setting
/// it as the default blueprint. It does look bruteforce, but it is strictly equivalent to loading
/// related RRDs which each contain a blueprint (e.g. because `rr.send_blueprint()` was called).
///
/// A key advantage of this approach is that it ensures that the default blueprint is always in sync
/// with the server's version.
pub async fn stream_blueprint_and_partition_from_server(
    mut client: ConnectionClient,
    tx: re_smart_channel::Sender<LogMsg>,
    uri: re_uri::DatasetDataUri,
    on_cmd: Box<dyn Fn(Command) + Send + Sync>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    re_log::debug!("Loading {uri}…");

    let response: ReadDatasetEntryResponse = client
        .inner()
        .read_dataset_entry(ReadDatasetEntryRequest {
            id: Some(uri.dataset_id.into()),
        })
        .await?
        .into_inner()
        .try_into()?;

    if let Some((blueprint_dataset, blueprint_partition)) =
        response.dataset_entry.dataset_details.default_bluprint()
    {
        re_log::debug!("Streaming blueprint dataset {blueprint_dataset}");

        // It may be tempting to use the partition id to build the `StoreId` here, but we require
        // store ids to be unique within a Viewer session (see e.g. `StoreBundle`), and partition
        // ids are only unique within a given dataset.
        // This is a hack be cause
        // TODO(#7950)
        let blueprint_store_id = StoreId::random(StoreKind::Blueprint);

        let blueprint_store_info = StoreInfo {
            application_id: uri.dataset_id.to_string().into(),
            store_id: blueprint_store_id.clone(),
            cloned_from: None,
            store_source: StoreSource::Unknown,
            store_version: None,
        };

        stream_partition_from_server(
            &mut client,
            blueprint_store_info,
            &tx,
            blueprint_dataset,
            blueprint_partition,
            None,
            &on_cmd,
            on_msg.as_deref(),
        )
        .await?;

        if tx
            .send(LogMsg::BlueprintActivationCommand(
                BlueprintActivationCommand {
                    blueprint_id: blueprint_store_id,
                    make_active: false,
                    make_default: true,
                },
            ))
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }
    } else {
        re_log::debug!("No blueprint dataset found for {uri}");
    }

    let store_info = StoreInfo {
        application_id: uri.dataset_id.to_string().into(),
        // See note above about `StoreId::random`.
        store_id: StoreId::random(StoreKind::Recording),
        cloned_from: None,
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    let re_uri::DatasetDataUri {
        origin: _,
        dataset_id,
        partition_id,
        time_range,
        fragment: _,
    } = uri;

    stream_partition_from_server(
        &mut client,
        store_info,
        &tx,
        dataset_id.into(),
        partition_id.into(),
        time_range,
        &on_cmd,
        on_msg.as_deref(),
    )
    .await?;

    Ok(())
}

/// Low-level function to stream data as a chunk store from a server.
#[expect(clippy::too_many_arguments)]
async fn stream_partition_from_server(
    client: &mut ConnectionClient,
    store_info: StoreInfo,
    tx: &re_smart_channel::Sender<LogMsg>,
    dataset_id: EntryId,
    partition_id: PartitionId,
    time_range: Option<TimeRange>,
    on_cmd: &(dyn Fn(Command) + Send + Sync),
    on_msg: Option<&(dyn Fn() + Send + Sync)>,
) -> Result<(), StreamError> {
    let catalog_chunk_stream = client
        .inner()
        .get_chunks(GetChunksRequest {
            dataset_id: Some(dataset_id.into()),
            partition_ids: vec![partition_id.into()],
            chunk_ids: vec![],
            entity_paths: vec![],
            query: None,
        })
        .await?
        .into_inner();

    let store_id = store_info.store_id.clone();

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

    // TODO(#10229): this looks to be converting back and forth?

    let mut chunk_stream = get_chunks_response_to_chunk_and_partition_id(catalog_chunk_stream);

    while let Some(chunks) = chunk_stream.next().await {
        for chunk in chunks? {
            let (chunk, _partition_id) = chunk;

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
    }

    Ok(())
}
