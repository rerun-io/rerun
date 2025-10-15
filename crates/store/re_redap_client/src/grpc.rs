use re_auth::client::AuthDecorator;
use re_chunk::Chunk;
use re_log_types::{
    AbsoluteTimeRange, BlueprintActivationCommand, DataSourceMessage, DataSourceUiCommand, EntryId,
    LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_protos::cloud::v1alpha1::ext::{Query, QueryLatestAt, QueryRange};
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::common::v1alpha1::ext::PartitionId;
use re_uri::{Origin, TimeSelection};

use tokio_stream::{Stream, StreamExt as _};

use crate::{ApiError, ConnectionClient, MAX_DECODING_MESSAGE_SIZE};

#[cfg(target_arch = "wasm32")]
pub async fn channel(origin: Origin) -> Result<tonic_web_wasm_client::Client, ApiError> {
    let channel = tonic_web_wasm_client::Client::new_with_options(
        origin.as_url(),
        tonic_web_wasm_client::options::FetchOptions::new(),
    );

    Ok(channel)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn channel(origin: Origin) -> Result<tonic::transport::Channel, ApiError> {
    use std::net::Ipv4Addr;

    use tonic::transport::Endpoint;

    let http_url = origin.as_url();

    let endpoint = {
        let mut endpoint = Endpoint::new(http_url)
            .and_then(|ep| {
                ep.tls_config(
                    tonic::transport::ClientTlsConfig::new()
                        .with_enabled_roots()
                        .assume_http2(true),
                )
            })
            .map_err(|err| ApiError::connection(err, "connecting to server"))?;

        if false {
            // NOTE: Tried it, had no noticeable effects in any of my benchmarks.
            endpoint = endpoint.initial_stream_window_size(Some(4 * 1024 * 1024));
            endpoint = endpoint.initial_connection_window_size(Some(16 * 1024 * 1024));
        }

        endpoint
            .connect()
            .await
            .map_err(|err| ApiError::connection(err, "connecting to server"))
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
                return Err(original_error);
            }

            // If we can't establish a connection, we probe if the server is
            // expecting unencrypted traffic. If that is the case, we return
            // a more meaningful error message.
            let Ok(endpoint) = Endpoint::new(origin.coerce_http_url()) else {
                return Err(original_error);
            };

            if endpoint.connect().await.is_ok() {
                Err(ApiError {
                    message: "server is expecting an unencrypted connection (try `rerun+http://` if you are sure)".to_owned(),
                    kind: crate::ApiErrorKind::Connection,
                    source: None,
                })
            } else {
                Err(original_error)
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub type RedapClientInner = tonic::service::interceptor::InterceptedService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<tonic_web_wasm_client::Client>,
        re_protos::headers::RerunVersionInterceptor,
    >,
    re_auth::client::AuthDecorator,
>;

#[cfg(target_arch = "wasm32")]
pub(crate) async fn client(
    origin: Origin,
    token: Option<re_auth::Jwt>,
) -> Result<RedapClient, ApiError> {
    let channel = channel(origin).await?;

    let auth = AuthDecorator::new(token);

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::InterceptorLayer::new(auth))
        .layer({
            let name = Some("rerun-web".to_owned());
            let version = None;
            let is_client = true;
            re_protos::headers::new_rerun_headers_layer(name, version, is_client)
        });

    let svc = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    Ok(RerunCloudServiceClient::new(svc).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
pub type RedapClientInner = tonic::service::interceptor::InterceptedService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<
            re_perf_telemetry::external::tower_http::trace::Trace<
                tonic::service::interceptor::InterceptedService<
                    tonic::transport::Channel,
                    re_perf_telemetry::TracingInjectorInterceptor,
                >,
                re_perf_telemetry::external::tower_http::classify::SharedClassifier<
                    re_perf_telemetry::external::tower_http::classify::GrpcErrorsAsFailures,
                >,
                re_perf_telemetry::GrpcMakeSpan,
            >,
        >,
        re_protos::headers::RerunVersionInterceptor,
    >,
    re_auth::client::AuthDecorator,
>;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "perf_telemetry")))]
pub type RedapClientInner = tonic::service::interceptor::InterceptedService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<tonic::transport::Channel>,
        re_protos::headers::RerunVersionInterceptor,
    >,
    re_auth::client::AuthDecorator,
>;

pub type RedapClient = RerunCloudServiceClient<RedapClientInner>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn client(
    origin: Origin,
    token: Option<re_auth::Jwt>,
) -> Result<RedapClient, ApiError> {
    let channel = channel(origin).await?;

    let middlewares = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::InterceptorLayer::new(
            AuthDecorator::new(token),
        ))
        .layer({
            let name = None;
            let version = None;
            let is_client = true;
            re_protos::headers::new_rerun_headers_layer(name, version, is_client)
        });

    #[cfg(feature = "perf_telemetry")]
    let middlewares = middlewares.layer(re_perf_telemetry::new_client_telemetry_layer());

    let svc = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    Ok(RerunCloudServiceClient::new(svc).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

/// Converts a `FetchChunksStream` stream into a stream of `Chunk`s.
//
// TODO(#9430): ideally this should be factored as a nice helper in `re_proto`
// TODO(cmc): we should compute contiguous runs of the same partition here, and return a `(String, Vec<Chunk>)`
// instead. Because of how the server performs the computation, this will very likely work out well
// in practice.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_chunks_response_to_chunk_and_partition_id<S>(
    response: S,
) -> impl Stream<Item = Result<Vec<(Chunk, Option<String>)>, ApiError>>
where
    S: Stream<Item = Result<re_protos::cloud::v1alpha1::FetchChunksResponse, tonic::Status>>,
{
    response
        .then(|resp| {
            // We want to make sure to offload that compute-heavy work to the compute worker pool: it's
            // not going to make this one single pipeline any faster, but it will prevent starvation of
            // the Tokio runtime (which would slow down every other futures currently scheduled!).
            tokio::task::spawn_blocking(move || {
                let r = resp.map_err(|err| ApiError::tonic(err, "failed to get item in /FetchChunks response stream"))?;
                let _span =
                    tracing::trace_span!("fetch_chunks::batch_decode", num_chunks = r.chunks.len())
                        .entered();

                r.chunks
                    .into_iter()
                    .map(|arrow_msg| {
                        let partition_id = arrow_msg.store_id.clone().map(|id| id.recording_id);

                        let arrow_msg =
                            re_log_encoding::protobuf_conversions::arrow_msg_from_proto(&arrow_msg)
                                .map_err(|err| ApiError::serialization(err, "failed to get arrow data for item in /FetchChunks response stream"))?;

                        let chunk = re_chunk::Chunk::from_record_batch(&arrow_msg.batch)
                            .map_err(|err| ApiError::serialization(err, "failed to parse item in /FetchChunks response stream"))?;

                        Ok((chunk, partition_id))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .map(|res| {
            res.map_err(|err| ApiError::internal(err, "failed to sync on /FetchChunks response stream"))
                .and_then(std::convert::identity)
        })
}

// This code path happens to be shared between native and web, but we don't have a Tokio runtime on web!
#[cfg(target_arch = "wasm32")]
pub fn fetch_chunks_response_to_chunk_and_partition_id<S>(
    response: S,
) -> impl Stream<Item = Result<Vec<(Chunk, Option<String>)>, ApiError>>
where
    S: Stream<Item = Result<re_protos::cloud::v1alpha1::FetchChunksResponse, tonic::Status>>,
{
    response.map(|resp| {
        let resp = resp.map_err(|err| {
            ApiError::tonic(err, "failed to get item in /FetchChunks response stream")
        })?;

        let _span =
            tracing::trace_span!("fetch_chunks::batch_decode", num_chunks = resp.chunks.len())
                .entered();

        resp.chunks
            .into_iter()
            .map(|arrow_msg| {
                let partition_id = arrow_msg.store_id.clone().map(|id| id.recording_id);

                let arrow_msg =
                    re_log_encoding::protobuf_conversions::arrow_msg_from_proto(&arrow_msg)
                        .map_err(|err| {
                            ApiError::serialization(
                                err,
                                "failed to get arrow data for item in /FetchChunks response stream",
                            )
                        })?;

                let chunk =
                    re_chunk::Chunk::from_record_batch(&arrow_msg.batch).map_err(|err| {
                        ApiError::serialization(
                            err,
                            "failed to parse item in /FetchChunks response stream",
                        )
                    })?;

                Ok((chunk, partition_id))
            })
            .collect::<Result<Vec<_>, _>>()
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
    tx: re_smart_channel::Sender<DataSourceMessage>,
    uri: re_uri::DatasetPartitionUri,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), ApiError> {
    re_log::debug!("Loading {uri}…");

    let dataset_entry = client.read_dataset_entry(uri.dataset_id.into()).await?;

    let recording_store_id = uri.store_id();

    if let Some((blueprint_dataset, blueprint_partition)) =
        dataset_entry.dataset_details.default_bluprint()
    {
        re_log::debug!("Streaming blueprint dataset {blueprint_dataset}");

        // For blueprint, we can use a random recording ID
        let blueprint_store_id = StoreId::random(
            StoreKind::Blueprint,
            recording_store_id.application_id().clone(),
        );

        let blueprint_store_info = StoreInfo {
            store_id: blueprint_store_id.clone(),
            cloned_from: None,
            store_source: StoreSource::Unknown,
            store_version: None,
            is_partial: false,
        };

        stream_partition_from_server(
            &mut client,
            blueprint_store_info,
            &tx,
            blueprint_dataset,
            blueprint_partition,
            None,
            re_uri::Fragment::default(),
            on_msg.as_deref(),
        )
        .await?;

        if tx
            .send(
                LogMsg::BlueprintActivationCommand(BlueprintActivationCommand {
                    blueprint_id: blueprint_store_id,
                    make_active: false,
                    make_default: true,
                })
                .into(),
            )
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }
    } else {
        re_log::debug!("No blueprint dataset found for {uri}");
    }

    let re_uri::DatasetPartitionUri {
        origin: _,
        dataset_id,
        partition_id,
        time_range,
        fragment,
    } = uri;

    let store_info = StoreInfo {
        store_id: recording_store_id,
        cloned_from: None,
        store_source: StoreSource::Unknown,
        store_version: None,
        is_partial: time_range.is_some(),
    };

    stream_partition_from_server(
        &mut client,
        store_info,
        &tx,
        dataset_id.into(),
        partition_id.into(),
        time_range,
        fragment,
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
    tx: &re_smart_channel::Sender<DataSourceMessage>,
    dataset_id: EntryId,
    partition_id: PartitionId,
    time_range: Option<TimeSelection>,
    fragment: re_uri::Fragment,
    on_msg: Option<&(dyn Fn() + Send + Sync)>,
) -> Result<(), ApiError> {
    let exclude_static_data = false;
    let exclude_temporal_data = true;
    let static_chunk_stream = client
        .fetch_partition_chunks(
            dataset_id,
            partition_id.clone(),
            exclude_static_data,
            exclude_temporal_data,
            None,
        )
        .await?;

    let exclude_static_data = true;
    let exclude_temporal_data = false;
    let temporal_chunk_stream = client
        .fetch_partition_chunks(
            dataset_id,
            partition_id,
            exclude_static_data,
            exclude_temporal_data,
            time_range.clone().map(|time_range| {
                Query {
                    range: Some(QueryRange {
                        index: time_range.timeline.name().to_string(),
                        index_range: time_range.clone().into(),
                    }),
                    latest_at: Some(QueryLatestAt {
                        index: Some(time_range.timeline.name().to_string()),
                        at: time_range.range.min(),
                    }),
                    columns_always_include_everything: false,
                    columns_always_include_chunk_ids: false,
                    columns_always_include_byte_offsets: false,
                    columns_always_include_entity_paths: false,
                    columns_always_include_static_indexes: false,
                    columns_always_include_global_indexes: false,
                    columns_always_include_component_indexes: false,
                }
                .into()
            }),
        )
        .await?;

    let store_id = store_info.store_id.clone();

    if tx
        .send(
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *re_chunk::RowId::new(),
                info: store_info,
            })
            .into(),
        )
        .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(());
    }

    // Send UI commands for recording (as opposed to blueprint) stores.
    if store_id.is_recording() {
        let valid_range_msg = if let Some(time_range) = time_range {
            DataSourceUiCommand::AddValidTimeRange {
                store_id: store_id.clone(),
                timeline: Some(*time_range.timeline.name()),
                time_range: time_range.into(),
            }
        } else {
            DataSourceUiCommand::AddValidTimeRange {
                store_id: store_id.clone(),
                timeline: None,
                time_range: AbsoluteTimeRange::EVERYTHING,
            }
        };

        if tx.send(valid_range_msg.into()).is_err() {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }

        #[expect(clippy::collapsible_if)]
        if !fragment.is_empty() {
            if tx
                .send(
                    DataSourceUiCommand::SetUrlFragment {
                        store_id: store_id.clone(),
                        fragment: fragment.to_string(),
                    }
                    .into(),
                )
                .is_err()
            {
                re_log::debug!("Receiver disconnected");
                return Ok(());
            }
        }
    }

    // TODO(#10229): this looks to be converting back and forth?

    let static_chunk_stream = fetch_chunks_response_to_chunk_and_partition_id(static_chunk_stream);
    let temporal_chunk_stream =
        fetch_chunks_response_to_chunk_and_partition_id(temporal_chunk_stream);

    let mut chunk_stream = static_chunk_stream.chain(temporal_chunk_stream);

    while let Some(chunks) = chunk_stream.next().await {
        for chunk in chunks? {
            let (chunk, _partition_id) = chunk;

            if tx
                .send(
                    LogMsg::ArrowMsg(
                        store_id.clone(),
                        chunk.to_arrow_msg().map_err(|err| {
                            ApiError::serialization(
                                err,
                                "failed to parse chunk in /FetchChunks response stream",
                            )
                        })?,
                    )
                    .into(),
                )
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
