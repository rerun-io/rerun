use std::ops::ControlFlow;
use std::sync::Arc;

use arrow::array::{AsArray as _, RecordBatch};
use arrow::error::ArrowError;
use re_auth::client::AuthDecorator;
use re_chunk::{Chunk, ChunkId};
use re_log_channel::{DataSourceMessage, DataSourceUiCommand};
use re_log_types::{
    BlueprintActivationCommand, EntryId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind,
    StoreSource,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_uri::Origin;
use tokio_stream::{Stream, StreamExt as _};

use crate::{
    ApiError, ApiErrorKind, ApiResult, ConnectionClient, MAX_DECODING_MESSAGE_SIZE,
    SegmentQueryParams, StreamMode,
};

#[cfg(target_arch = "wasm32")]
pub async fn channel(origin: Origin) -> ApiResult<tonic_web_wasm_client::Client> {
    let channel = tonic_web_wasm_client::Client::new_with_options(
        origin.as_url(),
        tonic_web_wasm_client::options::FetchOptions::new(),
    );

    Ok(channel)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn channel(origin: Origin) -> ApiResult<tonic::transport::Channel> {
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
            .map_err(|err| ApiError::connection(err, "connecting to server"))?
            .http2_adaptive_window(true) // Optimize for throughput
            .connect_timeout(std::time::Duration::from_secs(10));

        if false {
            // NOTE: Tried it, had no noticeable effects in any of my benchmarks.
            endpoint = endpoint.initial_stream_window_size(Some(4 * 1024 * 1024));
            endpoint = endpoint.initial_connection_window_size(Some(16 * 1024 * 1024));
        }

        endpoint.connect().await.map_err(|err| {
            ApiError::connection(err, format!("failed to connect to server at {origin}"))
        })
    };

    match endpoint {
        Ok(channel) => Ok(channel),
        Err(original_error) => {
            if ![
                url::Host::Domain("localhost".to_owned()),
                url::Host::Ipv4(Ipv4Addr::LOCALHOST),
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

            let endpoint = endpoint.http2_adaptive_window(true); // Optimize for throughput

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
pub type RedapClientInner = re_auth::client::AuthService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<tonic_web_wasm_client::Client>,
        re_protos::headers::RerunVersionInterceptor,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub(crate) async fn client(
    origin: Origin,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
) -> ApiResult<RedapClient> {
    let channel = channel(origin).await?;

    let middlewares = tower::ServiceBuilder::new()
        .layer(AuthDecorator::new(credentials))
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
pub type RedapClientInner = re_auth::client::AuthService<
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
>;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "perf_telemetry")))]
pub type RedapClientInner = re_auth::client::AuthService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<tonic::transport::Channel>,
        re_protos::headers::RerunVersionInterceptor,
    >,
>;

pub type RedapClient = RerunCloudServiceClient<RedapClientInner>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn client(
    origin: Origin,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
) -> ApiResult<RedapClient> {
    let channel = channel(origin).await?;

    let middlewares = tower::ServiceBuilder::new()
        .layer(AuthDecorator::new(credentials))
        .layer({
            let name = None;
            let version = None;
            let is_client = true;
            re_protos::headers::new_rerun_headers_layer(name, version, is_client)
        });

    #[cfg(feature = "perf_telemetry")]
    let middlewares = middlewares.layer(re_perf_telemetry::new_client_telemetry_layer());

    let svc: RedapClientInner = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    Ok(RerunCloudServiceClient::new(svc).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE))
}

/// Converts a `FetchChunksStream` stream into a stream of `Chunk`s.
//
// TODO(#9430): ideally this should be factored as a nice helper in `re_proto`
// TODO(cmc): we should compute contiguous runs of the same segment here, and return a `(String, Vec<Chunk>)`
// instead. Because of how the server performs the computation, this will very likely work out well
// in practice.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_chunks_response_to_chunk_and_segment_id<S>(
    response: S,
) -> impl Stream<Item = ApiResult<Vec<(Chunk, Option<String>)>>>
where
    S: Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::FetchChunksResponse>>,
{
    response
        .then(|resp| {
            // We want to make sure to offload that compute-heavy work to the compute worker pool: it's
            // not going to make this one single pipeline any faster, but it will prevent starvation of
            // the Tokio runtime (which would slow down every other futures currently scheduled!).
            tokio::task::spawn_blocking(move || {
                let r = resp.map_err(|err| {
                    ApiError::tonic(err, "failed to get item in /FetchChunks response stream")
                })?;
                let _span =
                    tracing::trace_span!("fetch_chunks::batch_decode", num_chunks = r.chunks.len())
                        .entered();

                r.chunks
                    .into_iter()
                    .map(|arrow_msg| {
                        let segment_id = arrow_msg.store_id.clone().map(|id| id.recording_id);

                        use re_log_encoding::ToApplication as _;
                        let arrow_msg = arrow_msg.to_application(()).map_err(|err| {
                            ApiError::serialization(
                                err,
                                "failed to get arrow data for item in /FetchChunks response stream",
                            )
                        })?;

                        let chunk = re_chunk::Chunk::from_record_batch(&arrow_msg.batch).map_err(
                            |err| {
                                ApiError::serialization(
                                    err,
                                    "failed to parse item in /FetchChunks response stream",
                                )
                            },
                        )?;

                        Ok((chunk, segment_id))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .map(|res| {
            res.map_err(|err| {
                ApiError::internal(err, "failed to sync on /FetchChunks response stream")
            })
            .and_then(std::convert::identity)
        })
}

// This code path happens to be shared between native and web, but we don't have a Tokio runtime on web!
#[cfg(target_arch = "wasm32")]
pub fn fetch_chunks_response_to_chunk_and_segment_id<S>(
    response: S,
) -> impl Stream<Item = ApiResult<Vec<(Chunk, Option<String>)>>>
where
    S: Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::FetchChunksResponse>>,
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
                let segment_id = arrow_msg.store_id.clone().map(|id| id.recording_id);

                use re_log_encoding::ToApplication as _;
                let arrow_msg = arrow_msg.to_application(()).map_err(|err| {
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

                Ok((chunk, segment_id))
            })
            .collect::<Result<Vec<_>, _>>()
    })
}

/// Canonical way to ingest segment data from a Rerun data platform server, dealing with
/// server-stored blueprints if any.
///
/// The current strategy currently consists of _always_ downloading the blueprint first and setting
/// it as the default blueprint. It does look bruteforce, but it is strictly equivalent to loading
/// related RRDs which each contain a blueprint (e.g. because `rr.send_blueprint()` was called).
///
/// A key advantage of this approach is that it ensures that the default blueprint is always in sync
/// with the server's version.
///
/// `stream_mode` is a feature-flag for RRD manifest based larger-than-ram streaming.
pub async fn stream_blueprint_and_segment_from_server(
    mut client: ConnectionClient,
    tx: re_log_channel::LogSender,
    uri: re_uri::DatasetSegmentUri,
    stream_mode: StreamMode,
) -> ApiResult {
    re_log::debug!("Loading {uri}…");

    let dataset_entry = client.read_dataset_entry(uri.dataset_id.into()).await?;

    let recording_store_id = uri.store_id();

    if let Some((blueprint_dataset, blueprint_segment)) =
        dataset_entry.dataset_details.default_blueprint()
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
        };

        if stream_segment_from_server(
            &mut client,
            blueprint_store_info,
            &tx,
            blueprint_dataset,
            blueprint_segment,
            re_uri::Fragment::default(),
            StreamMode::FullLoad, // We always load the full blueprint
        )
        .await?
        .is_break()
        {
            return Ok(());
        }

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

    let re_uri::DatasetSegmentUri {
        origin: _,
        dataset_id,
        segment_id,
        fragment,
    } = uri;

    let store_info = StoreInfo {
        store_id: recording_store_id,
        cloned_from: None,
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    if stream_segment_from_server(
        &mut client,
        store_info,
        &tx,
        dataset_id.into(),
        segment_id.into(),
        fragment,
        stream_mode,
    )
    .await?
    .is_break()
    {
        return Ok(());
    }

    Ok(())
}

/// Low-level function to stream data as a chunk store from a server.
async fn stream_segment_from_server(
    client: &mut ConnectionClient,
    store_info: StoreInfo,
    tx: &re_log_channel::LogSender,
    dataset_id: EntryId,
    segment_id: SegmentId,
    fragment: re_uri::Fragment,
    stream_mode: StreamMode,
) -> ApiResult<ControlFlow<()>> {
    let store_id = store_info.store_id.clone();

    re_log::debug!("Streaming {store_id:?}…");

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
        return Ok(ControlFlow::Break(()));
    }

    // Send UI commands for recording (as opposed to blueprint) stores.
    #[expect(clippy::collapsible_if)]
    if store_id.is_recording() && !fragment.is_empty() {
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
            return Ok(ControlFlow::Break(()));
        }
    }

    // TODO(RR-2976): Do not, under any circumstances, try to chain gRPC streams
    // together. Interlaced streams are a giant footgun that will invariably lead to the exhaustion
    // of client's HTTP2 connection window, and ultimately to a complete stall of the entire system.
    // See the attached issues for more information.

    if stream_mode == StreamMode::OnDemand {
        let manifest_result = client
            .get_rrd_manifest(dataset_id, segment_id.clone())
            .await;
        match manifest_result {
            Ok(rrd_manifest) => {
                re_log::debug_once!("The server supports larger-than-RAM");

                if tx
                    .send(DataSourceMessage::RrdManifest(
                        store_id.clone(),
                        rrd_manifest.clone().into(),
                    ))
                    .is_err()
                {
                    re_log::debug!("Receiver disconnected");
                    return Ok(ControlFlow::Break(()));
                }

                if store_id.is_recording() {
                    return load_static_chunks(client, tx, &store_id, rrd_manifest).await;
                } else {
                    // Load all chunks in one go; most important first:
                    let batch = sort_batch(&rrd_manifest.data).map_err(|err| {
                        ApiError::invalid_arguments(err, "Failed to sort chunk index")
                    })?;
                    return load_chunks(client, tx, &store_id, batch).await;
                }
            }
            Err(err) => {
                if err.kind == ApiErrorKind::Unimplemented {
                    re_log::debug_once!("The server does not support larger-than-RAM"); // Legacy server
                } else {
                    re_log::warn!("Failed to load RRD manifest: {err}");
                }
            }
        }
    } else {
        re_log::debug_once!("Larger-than-RAM streaming is disabled");
    }

    // Fallback for servers that does not support the RRD manifests:

    let mut already_loaded_chunk_ids: ahash::HashSet<ChunkId> = Default::default();

    if let Some(time_selection) = fragment.time_selection {
        // Start by loading only the chunks required for the time selection:
        let time_selection_batches = client
            .query_dataset_chunk_index(SegmentQueryParams {
                dataset_id,
                segment_id: segment_id.clone(),
                include_static_data: true,
                include_temporal_data: true,
                query: Some(
                    re_protos::cloud::v1alpha1::ext::Query::latest_at_range(
                        time_selection.timeline.name(),
                        time_selection.range,
                    )
                    .into(),
                ),
            })
            .await?;

        if time_selection_batches.is_empty() {
            re_log::debug!(
                "No chunks found for time selection {:?} in recording {:?}",
                time_selection,
                store_id
            );
        } else {
            let batch = arrow::compute::concat_batches(
                &time_selection_batches[0].schema(),
                &time_selection_batches,
            )
            .map_err(|err| {
                ApiError::invalid_arguments(err, "Failed to concat chunk index batches")
            })?;

            // Prioritize the chunks:
            let batch = sort_batch(&batch)
                .map_err(|err| ApiError::invalid_arguments(err, "Failed to sort chunk index"))?;

            if let Some(chunk_ids) = chunk_id_column(&batch) {
                already_loaded_chunk_ids = chunk_ids.iter().copied().collect();
            } else {
                re_log::warn_once!(
                    "Failed to find 'chunk_id' column in chunk index response. Schema: {}",
                    batch.schema()
                );
            }

            if load_chunks(client, tx, &store_id, batch).await?.is_break() {
                return Ok(ControlFlow::Break(()));
            }
        }

        // Now load the rest (chunks outside the time range):
    }

    let batches = client
        .query_dataset_chunk_index(SegmentQueryParams {
            dataset_id,
            segment_id: segment_id.clone(),
            include_static_data: true,
            include_temporal_data: true,
            query: None, // everything
        })
        .await?;

    if batches.is_empty() {
        re_log::info!("Empty recording"); // We likely won't get here even on empty recording
        return Ok(ControlFlow::Continue(()));
    }

    let batch = arrow::compute::concat_batches(&batches[0].schema(), &batches)
        .map_err(|err| ApiError::invalid_arguments(err, "Failed to concat chunk index batches"))?;

    // Prioritize the chunks:
    let batch = sort_batch(&batch)
        .map_err(|err| ApiError::invalid_arguments(err, "Failed to sort chunk index"))?;

    if let Some(chunk_ids) = chunk_id_column(&batch)
        && !already_loaded_chunk_ids.is_empty()
    {
        // Filter out already loaded chunk IDs:
        let filtered_indices: Vec<usize> = chunk_ids
            .iter()
            .enumerate()
            .filter_map(|(idx, chunk_id)| {
                if already_loaded_chunk_ids.contains(chunk_id) {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect();

        let filtered_batch = arrow::compute::take_record_batch(
            &batch,
            &arrow::array::UInt32Array::from(
                filtered_indices
                    .iter()
                    .map(|&i| i as u32)
                    .collect::<Vec<u32>>(),
            ),
        )
        .map_err(|err| ApiError::invalid_arguments(err, "take_record_batch"))?;

        load_chunks(client, tx, &store_id, filtered_batch).await
    } else {
        load_chunks(client, tx, &store_id, batch).await
    }
}

fn chunk_id_column(batch: &RecordBatch) -> Option<&[ChunkId]> {
    batch
        .column_by_name("chunk_id")
        .and_then(|array| array.as_fixed_size_binary_opt())
        .and_then(|array| ChunkId::try_slice_from_arrow(array).ok())
}

/// Load only static chunks
async fn load_static_chunks(
    client: &mut ConnectionClient,
    tx: &re_log_channel::LogSender,
    store_id: &StoreId,
    rrd_manifest: re_log_encoding::RrdManifest,
) -> ApiResult<ControlFlow<()>> {
    let col_chunk_is_static = rrd_manifest
        .col_chunk_is_static()
        .map_err(|err| ApiError::internal(err, "RRD Manifest missing chunk_is_static column"))?;

    let mut indices = vec![];
    for (row_idx, chunk_is_static) in col_chunk_is_static.enumerate() {
        if chunk_is_static {
            indices.push(row_idx as u32);
        }
    }
    let static_chunks = arrow::compute::take_record_batch(
        &rrd_manifest.data,
        &arrow::array::UInt32Array::from(indices),
    )
    .map_err(|err| ApiError::internal(err, "take_record_batch"))?;

    re_log::debug!(
        "Pre-fetching {} static chunks…",
        re_format::format_uint(static_chunks.num_rows())
    );
    if load_chunks(client, tx, store_id, static_chunks)
        .await?
        .is_break()
    {
        return Ok(ControlFlow::Break(()));
    }

    re_log::debug!(
        "All static chunks have been loaded. Letting the viewer manually load the rest of the chunks it wants."
    );

    Ok(ControlFlow::Break(()))
}

/// Takes a dataframe that looks like an [`re_log_encoding::RrdManifest`] (has a `chunk_key` column).
async fn load_chunks(
    client: &mut ConnectionClient,
    tx: &re_log_channel::LogSender,
    store_id: &StoreId,
    batch: RecordBatch,
) -> ApiResult<ControlFlow<()>> {
    if batch.num_rows() == 0 {
        return Ok(ControlFlow::Continue(()));
    }

    re_log::trace!("Requesting {} chunks from server…", batch.num_rows());

    let chunk_stream = client.fetch_segment_chunks_by_id(&batch).await?;
    let mut chunk_stream = fetch_chunks_response_to_chunk_and_segment_id(chunk_stream);
    while let Some(chunks) = chunk_stream.next().await {
        for (chunk, _partition_id) in chunks? {
            if tx
                .send(
                    LogMsg::ArrowMsg(
                        store_id.clone(),
                        // TODO(#10229): this looks to be converting back and forth?
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
                return Ok(ControlFlow::Break(()));
            }
        }
    }

    re_log::trace!("Finished downloading {} chunks.", batch.num_rows());

    Ok(ControlFlow::Continue(()))
}

fn sort_batch(batch: &RecordBatch) -> Result<RecordBatch, ArrowError> {
    use std::sync::Arc;

    let schema = batch.schema();

    // Get column indices:
    let chunk_is_static = schema.index_of("chunk_is_static")?;
    let chunk_id = schema.index_of("chunk_id")?;

    let sort_keys = vec![
        // Static first:
        arrow::compute::SortColumn {
            values: Arc::new(batch.column(chunk_is_static).clone()),
            options: Some(arrow::compute::SortOptions {
                descending: true,
                nulls_first: true,
            }),
        },
        // Then sort by chunk id (~time)
        arrow::compute::SortColumn {
            values: Arc::new(batch.column(chunk_id).clone()),
            options: Some(arrow::compute::SortOptions {
                descending: false,
                nulls_first: true,
            }),
        },
    ];

    let indices = arrow::compute::lexsort_to_indices(&sort_keys, None)?;
    let sorted = arrow::compute::take_record_batch(batch, &indices)?;

    Ok(sorted)
}
