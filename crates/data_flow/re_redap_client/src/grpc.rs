use std::ops::ControlFlow;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

use arrow::array::RecordBatch;
use arrow::error::ArrowError;
use itertools::Itertools as _;
use re_auth::client::AuthDecorator;
use re_byte_size::SizeBytes as _;
use re_chunk::Chunk;
use re_chunk_index::{RawRrdManifest, RrdManifest};
use re_log_channel::{
    BlueprintTarget, DataSourceMessage, DefaultBlueprintRegistration, ViewerControlCommand,
};
use re_log_msg::{BlueprintActivationCommand, LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::{ApplicationId, EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::common::v1alpha1::ext::DatasetKind;
use re_types_core::SegmentId;
use re_uri::Origin;
use tokio_stream::StreamExt as _;

use crate::asset::{asset_manifest, asset_segments};
use crate::{
    ApiError, ApiResult, BoxedRedapClientStack, ConnectionClient, MAX_DECODING_MESSAGE_SIZE,
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
pub async fn channel(origin: Origin) -> ApiResult<PoolChannel> {
    use std::net::Ipv4Addr;

    use tonic::transport::Endpoint;

    let http_url = origin.as_url();

    let tls_config = if let Ok(cert_path) = std::env::var("RERUN_REDAP_LOCAL_CERT_PATH") {
        use tonic::transport::{Certificate, ClientTlsConfig};

        re_log::info!(cert_path, "starting client with local TLS cert");

        let ca_cert = tokio::fs::read_to_string(&cert_path).await.map_err(|err| {
            ApiError::internal_with_source(
                &origin,
                None,
                err,
                format!("couldn't load local cert at {cert_path:?}"),
            )
        })?;
        let ca_cert = Certificate::from_pem(ca_cert);

        ClientTlsConfig::new()
            .with_enabled_roots()
            .ca_certificate(ca_cert)
            .domain_name("localhost") // must match the Common Name (CN) in the self-signed cert
            .assume_http2(true)
    } else {
        // TODO(RR-3480): This will fail to connect to unencrypted IPv6 addresses (e.g. `rerun+http://[fd00:4b21:6f7a:2022::10]:51234`).
        tonic::transport::ClientTlsConfig::new()
            .with_enabled_roots()
            .assume_http2(true)
    };

    let mut endpoint = Endpoint::new(http_url)
        .and_then(|ep| ep.tls_config(tls_config))
        .map_err(|err| {
            ApiError::connection_with_source(&origin, None, err, "connecting to server")
        })?
        .http2_adaptive_window(true) // Optimize for throughput
        .connect_timeout(std::time::Duration::from_secs(10))
        // Send HTTP/2 PINGs every 30s to keep connections alive across NATs / cloud LBs
        // that silently drop idle TCP. Without a client-side keep-alive, idle long-lived
        // channels were torn down only when one side's keep-alive eventually fired,
        // surfacing as confusing "slow" requests on the next call.
        .http2_keep_alive_interval(std::time::Duration::from_secs(30))
        .keep_alive_timeout(std::time::Duration::from_secs(20))
        .keep_alive_while_idle(true)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)));

    if false {
        // NOTE: Tried it, had no noticeable effects in any of my benchmarks.
        endpoint = endpoint.initial_stream_window_size(Some(4 * 1024 * 1024));
        endpoint = endpoint.initial_connection_window_size(Some(16 * 1024 * 1024));
    }

    // Connect a single channel up front. This validates connectivity (so the localhost probe
    // below can run on failure) and surfaces connection errors through the `with_retry` wrapper.
    let connect_result = endpoint.connect().await.map_err(|err| {
        ApiError::connection_with_source(
            &origin,
            None,
            err,
            format!("failed to connect to server at {origin}"),
        )
    });

    match connect_result {
        // Spread requests across a small pool of connections to the same origin (see
        // `PoolChannel`). For a single-connection pool this is just the validated channel.
        Ok(channel) => Ok(build_pool(channel, &endpoint)),
        Err(original_err) => {
            if ![
                url::Host::Domain("localhost".to_owned()),
                url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            ]
            .contains(&origin.host)
            {
                return Err(original_err);
            }

            // If we can't establish a connection, we probe if the server is
            // expecting unencrypted traffic. If that is the case, we return
            // a more meaningful error message.
            let Ok(endpoint) = Endpoint::new(origin.coerce_http_url()) else {
                return Err(original_err);
            };

            let endpoint = endpoint.http2_adaptive_window(true); // Optimize for throughput

            if endpoint.connect().await.is_ok() {
                Err(ApiError::connection(
                    &origin,
                    "the server is expecting an unencrypted connection (try `rerun+http://` if you are sure)",
                ))
            } else {
                Err(original_err)
            }
        }
    }
}

/// Default number of HTTP/2 connections opened per origin (see [`connection_pool_size`]).
///
/// `4` by default. The viewer fans out many concurrent `FetchChunks` streams, and a single HTTP/2
/// connection serves them slowly: the server's HTTP/2 stack under-polls the multiplexed streams.
/// Spreading them across `4` connections keeps each one lightly loaded. `4` is the measured
/// sweet spot on a 80 MiB / s connection. Connections are opened lazily, so light usage still only
/// ever touches the first one.
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_CONNECTION_POOL_SIZE: usize = 4;

/// How many HTTP/2 connections to open per origin, for load-balancing concurrent requests.
///
/// Defaults to [`DEFAULT_CONNECTION_POOL_SIZE`]. Override with `RERUN_REDAP_CONNECTION_POOL_SIZE`.
///
/// Set it to `1` to disable pooling.
#[cfg(not(target_arch = "wasm32"))]
fn connection_pool_size() -> usize {
    std::env::var("RERUN_REDAP_CONNECTION_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CONNECTION_POOL_SIZE)
        .max(1)
}

/// Build a [`PoolChannel`] of `connection_pool_size()` connections to the same origin.
///
/// `validated` is the connection we already opened and checked above; it becomes the first pool
/// member. Any further connections are opened lazily, so they only cost a connect once the pool
/// actually routes a request to them. For a single-connection pool this is just `validated`.
#[cfg(not(target_arch = "wasm32"))]
fn build_pool(
    validated: tonic::transport::Channel,
    endpoint: &tonic::transport::Endpoint,
) -> PoolChannel {
    let pool_size = connection_pool_size();
    let mut connections = Vec::with_capacity(pool_size);
    connections.push(validated);
    for _ in 1..pool_size {
        connections.push(endpoint.connect_lazy());
    }
    PoolChannel::new(connections)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub enum GrpcTransport {
    Native,
    Web,
}

#[cfg(not(target_arch = "wasm32"))]
impl GrpcTransport {
    /// Returns the transport's HTTP media type without a message format suffix such as `+proto`.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Native => "application/grpc",
            Self::Web => "application/grpc-web",
        }
    }
}

/// A pool of HTTP/2 connections to a single origin, dispatching each request to the least-loaded
/// connection.
///
/// Channels are lazily opened, so under light usage only one connection is used.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct PoolChannel {
    connections: Arc<[tonic::transport::Channel]>,

    /// Number of requests on each connection whose response body has not yet finished streaming.
    in_flight: Arc<[std::sync::atomic::AtomicU64]>,
    transport: GrpcTransport,
}

#[cfg(not(target_arch = "wasm32"))]
impl PoolChannel {
    fn new(connections: Vec<tonic::transport::Channel>) -> Self {
        let transport = if re_log::env_var_is_truthy("RERUN_FORCE_GRPC_WEB") {
            re_log::warn_once!(
                "RERUN_FORCE_GRPC_WEB enables gRPC-Web for catalog requests. \
                 The client buffers each request in memory before sending it, so large writes can use more memory."
            );
            GrpcTransport::Web
        } else {
            GrpcTransport::Native
        };
        let in_flight = connections
            .iter()
            .map(|_| std::sync::atomic::AtomicU64::new(0))
            .collect::<Vec<_>>();
        Self {
            connections: connections.into(),
            in_flight: in_flight.into(),
            transport,
        }
    }

    /// Index of the connection with the fewest requests in flight.
    fn least_loaded(&self) -> usize {
        self.in_flight
            .iter()
            .enumerate()
            .min_by_key(|(_, n)| n.load(Ordering::Relaxed))
            .map_or(0, |(i, _)| i)
    }
}

/// Decrements a connection's in-flight count when dropped, i.e. once its response body is done
/// streaming.
#[cfg(not(target_arch = "wasm32"))]
struct InFlightGuard {
    in_flight: Arc<[std::sync::atomic::AtomicU64]>,
    idx: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.in_flight[self.idx].fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Wraps a response body to keep its request counted as in-flight until the body finishes streaming.
///
/// The guard is dropped when this body is fully drained or dropped, at which point the connection's
/// in-flight count is decremented (see [`InFlightGuard`]).
#[cfg(not(target_arch = "wasm32"))]
struct TrackedBody {
    inner: tonic::body::Body,
    _guard: InFlightGuard,
}

#[cfg(not(target_arch = "wasm32"))]
impl http_body::Body for TrackedBody {
    type Data = tonic::codegen::Bytes;
    type Error = tonic::Status;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn send_request<S>(
    transport: GrpcTransport,
    connection: S,
    request: tonic::codegen::http::Request<tonic::body::Body>,
) -> tonic::Result<tonic::codegen::http::Response<tonic::body::Body>>
where
    S: tower::Service<
            tonic::codegen::http::Request<tonic::body::Body>,
            Response = tonic::codegen::http::Response<tonic::body::Body>,
        > + Clone
        + Send
        + 'static,
    S::Error: Into<tonic::codegen::StdError>,
    S::Future: Send,
{
    use tower::ServiceExt as _;

    use crate::grpc_web_on_native;

    match transport {
        GrpcTransport::Web => grpc_web_on_native::send(connection, request).await,
        GrpcTransport::Native => {
            let response = connection
                .oneshot(request)
                .await
                .map_err(|err| tonic::Status::from_error(err.into()))?;
            check_response(response, transport)
        }
    }
}

/// Whether `response` has the given gRPC content type, with or without a `+proto`-style suffix.
#[cfg(not(target_arch = "wasm32"))]
fn is_grpc_response(
    response: &tonic::codegen::http::Response<tonic::body::Body>,
    content_type: &str,
) -> bool {
    response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value == content_type
                || value
                    .strip_prefix(content_type)
                    .is_some_and(|suffix| suffix.starts_with('+'))
        })
}

/// Turns a reply that is not a gRPC response into a [`tonic::Status`] that names the HTTP status.
///
/// Call this before tonic reads the body.
/// tonic decodes any body as gRPC frames, so an HTML or plain-text error page from a proxy becomes
/// a decoding error, such as `invalid compression flag: 109`, with code `Internal` whatever the
/// HTTP status was.
#[cfg(not(target_arch = "wasm32"))]
pub fn check_response(
    response: tonic::codegen::http::Response<tonic::body::Body>,
    transport: GrpcTransport,
) -> tonic::Result<tonic::codegen::http::Response<tonic::body::Body>> {
    let content_type = transport.content_type();
    if is_grpc_response(&response, content_type) || response.headers().contains_key("grpc-status") {
        return Ok(response);
    }
    // The mapping from the gRPC spec, which tonic also uses once a body ends without a status:
    // https://github.com/grpc/grpc/blob/9c3f79deb70460750421f3dd75c2ed6a9af91ca3/doc/http-grpc-status-mapping.md
    let code = match response.status() {
        http::StatusCode::BAD_REQUEST => tonic::Code::Internal,
        http::StatusCode::UNAUTHORIZED => tonic::Code::Unauthenticated,
        http::StatusCode::FORBIDDEN => tonic::Code::PermissionDenied,
        http::StatusCode::NOT_FOUND => tonic::Code::Unimplemented,
        http::StatusCode::TOO_MANY_REQUESTS
        | http::StatusCode::BAD_GATEWAY
        | http::StatusCode::SERVICE_UNAVAILABLE
        | http::StatusCode::GATEWAY_TIMEOUT => tonic::Code::Unavailable,
        _ => tonic::Code::Unknown,
    };
    Err(tonic::Status::new(
        code,
        format!(
            "server returned HTTP {} without an expected {content_type} response; check the server or proxy configuration",
            response.status(),
        ),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
impl tower::Service<tonic::codegen::http::Request<tonic::body::Body>> for PoolChannel {
    type Response = tonic::codegen::http::Response<tonic::body::Body>;
    type Error = tonic::Status;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        // We don't apply backpressure here: the chosen connection is driven to readiness inside the
        // returned future, and each connection does its own internal buffering.
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: tonic::codegen::http::Request<tonic::body::Body>) -> Self::Future {
        let idx = self.least_loaded();
        self.in_flight[idx].fetch_add(1, Ordering::Relaxed);
        let guard = InFlightGuard {
            in_flight: self.in_flight.clone(),
            idx,
        };
        let connection = self.connections[idx].clone();
        let transport = self.transport;

        Box::pin(async move {
            let response = send_request(transport, connection, req).await?;
            // Hand the in-flight guard to the response body so the request stays counted until the
            // body is fully streamed (or dropped), not just until the headers arrive. This keeps the
            // load metric tracking the bandwidth-bound download work, which is what actually needs
            // balancing across connections.
            Ok(response.map(|inner| {
                tonic::body::Body::new(TrackedBody {
                    inner,
                    _guard: guard,
                })
            }))
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub type RedapClientStack = re_auth::client::AuthService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<tonic_web_wasm_client::Client>,
        re_protos::headers::RerunVersionInterceptor,
    >,
>;

/// Apply the standard SDK-side layer stack on top of an already-built channel
/// and return the generated [`RawRedapClient`] plus the layered service backing it.
#[cfg(target_arch = "wasm32")]
pub(crate) fn assemble_grpc_client(
    channel: tonic_web_wasm_client::Client,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
    client_name: Option<String>,
) -> (RawRedapClient, RedapClientStack) {
    let middlewares = tower::ServiceBuilder::new()
        .layer(AuthDecorator::new(credentials))
        .layer(re_protos::headers::new_rerun_client_headers_layer(
            client_name,
        ));

    let client_stack: RedapClientStack = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    let client = redap_grpc_client(client_stack.clone());
    (client, client_stack)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn connect_grpc_client(
    origin: Origin,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
    client_name: Option<String>,
) -> ApiResult<(RawRedapClient, RedapClientStack)> {
    let channel = crate::with_retry("redap_connection", || async {
        channel(origin.clone()).await
    })
    .await?;
    Ok(assemble_grpc_client(channel, credentials, client_name))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
pub type RedapClientStack = re_auth::client::AuthService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<
            re_perf_telemetry::external::tower_http::trace::Trace<
                tonic::service::interceptor::InterceptedService<
                    PoolChannel,
                    re_perf_telemetry::TracingInjectorInterceptor,
                >,
                re_perf_telemetry::external::tower_http::classify::SharedClassifier<
                    re_perf_telemetry::external::tower_http::classify::GrpcErrorsAsFailures,
                >,
                re_perf_telemetry::GrpcMakeSpan,
                re_perf_telemetry::external::tower_http::trace::DefaultOnRequest,
                re_perf_telemetry::ClientOnResponse,
            >,
        >,
        re_protos::headers::RerunVersionInterceptor,
    >,
>;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "perf_telemetry")))]
pub type RedapClientStack = re_auth::client::AuthService<
    tonic::service::interceptor::InterceptedService<
        re_protos::headers::PropagateHeaders<PoolChannel>,
        re_protos::headers::RerunVersionInterceptor,
    >,
>;

pub(crate) type RawRedapClient = RerunCloudServiceClient<RedapClientStack>;

fn redap_grpc_client(client_stack: RedapClientStack) -> RawRedapClient {
    RerunCloudServiceClient::new(client_stack).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
}

pub fn boxed_redap_grpc_client<S, B>(service: S) -> RerunCloudServiceClient<BoxedRedapClientStack>
where
    S: tower::Service<
            tonic::codegen::http::Request<tonic::body::Body>,
            Response = tonic::codegen::http::Response<B>,
            Error = tonic::Status,
        > + Clone
        + Send
        + Sync
        + 'static,
    S::Future: Send + 'static,
    B: tonic::codegen::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError>,
{
    use tower::ServiceExt as _;

    let client_stack = tower::util::BoxCloneSyncService::new(
        tonic::service::interceptor::InterceptedService::new(service, |request| {
            Ok(crate::dataset_revisions().stamp(request))
        })
        .map_response(|response| response.map(tonic::body::Body::new)),
    );

    RerunCloudServiceClient::new(client_stack).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
}

/// Apply the standard SDK-side layer stack on top of an already-built channel
/// and return the generated [`RawRedapClient`] plus the layered service backing it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn assemble_grpc_client(
    channel: PoolChannel,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
    client_name: Option<String>,
) -> (RawRedapClient, RedapClientStack) {
    let middlewares = tower::ServiceBuilder::new()
        .layer(AuthDecorator::new(credentials))
        .layer(re_protos::headers::new_rerun_client_headers_layer(
            client_name,
        ));

    #[cfg(feature = "perf_telemetry")]
    let middlewares = middlewares.layer(re_perf_telemetry::new_client_telemetry_layer());

    let client_stack: RedapClientStack = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    let client = redap_grpc_client(client_stack.clone());
    (client, client_stack)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn connect_grpc_client(
    origin: Origin,
    credentials: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync + 'static>>,
    client_name: Option<String>,
) -> ApiResult<(RawRedapClient, RedapClientStack)> {
    let channel = crate::with_retry("redap_connection", || async {
        channel(origin.clone()).await
    })
    .await?;
    Ok(assemble_grpc_client(channel, credentials, client_name))
}

/// Decodes an [`ArrowMsg`](re_protos::log_msg::v1alpha1::ArrowMsg) into a chunk.
fn decode_chunk(
    arrow_msg: &re_protos::log_msg::v1alpha1::ArrowMsg,
    origin: &Origin,
    trace_id: Option<opentelemetry::TraceId>,
) -> Result<Chunk, ApiError> {
    re_tracing::profile_function!();

    use re_log_encoding::ToApplication as _;
    let arrow_msg = arrow_msg.to_application(()).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "failed to decode ArrowMsg payload",
        )
    })?;

    Chunk::from_arrow_msg(&arrow_msg).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "failed to parse chunk from ArrowMsg",
        )
    })
}

/// What the chunks decoded from a `/FetchChunks` response may keep alive.
///
/// An uncompressed chunk is decoded without copying, so its arrays alias the gRPC receive buffer,
/// which holds the whole response and can also hold neighboring responses.
/// Size estimates of a chunk (`heap_size_bytes`) do not include the rest of that buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchedPayloads {
    /// Chunks alias the receive buffer, which stays alive until every chunk sliced from it is dropped.
    ///
    /// Dropping only some of those chunks frees none of their bytes, so a memory budget that
    /// refunds the `heap_size_bytes` of dropped chunks overstates what was freed.
    Shared,

    /// Each uncompressed payload is copied out before decoding, so a chunk keeps alive only its
    /// own bytes (see [`re_protos::log_msg::v1alpha1::ArrowMsg::unshare_payload`]).
    ///
    /// For callers that retain chunks in a store whose garbage collection is sized by
    /// `heap_size_bytes`.
    Copied,
}

impl FetchedPayloads {
    fn detach(self, response: &mut [re_protos::log_msg::v1alpha1::ArrowMsg]) {
        if self == Self::Copied {
            re_tracing::profile_function!();
            for arrow_msg in response {
                arrow_msg.unshare_payload();
            }
        }
    }
}

/// Converts a `FetchChunksStream` stream into a stream of `Chunk`s.
///
/// The chunks do not say which segment they belong to: the recording id in their `store_id` is
/// that of the store they were logged to, which is not always the segment they were registered
/// as. The caller knows the segment from the rows it requested.
//
// TODO(#9430): ideally this should be factored as a nice helper in `re_proto`
#[tracing::instrument(level = "debug", skip_all)]
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_chunks_response_to_chunks(
    response: crate::FetchChunksResponseStream,
    payloads: FetchedPayloads,
    decode_time_us: Option<Arc<AtomicU64>>,
) -> crate::ApiResponseStream<Vec<Chunk>> {
    let origin = response.origin().clone();
    let trace_id = response.trace_id();

    // `spawn_blocking` runs on the blocking thread pool with no tracing context.
    // Capture the caller's span here so the decode/migration spans nested inside
    // are parented under the SDK call instead of becoming orphan roots in Jaeger.
    let parent_span = tracing::Span::current();
    let stream = response
        .then({
            let origin = origin.clone();
            move |resp| {
                let trace_id = trace_id;
                let origin = origin.clone();
                let parent_span = parent_span.clone();
                // Cheap `Option<Arc>` clone per response batch; only present when a
                // caller wants to attribute decode CPU time to a query.
                let decode_time_us = decode_time_us.clone();
                // We want to make sure to offload that compute-heavy work to the compute worker pool: it's
                // not going to make this one single pipeline any faster, but it will prevent starvation of
                // the Tokio runtime (which would slow down every other futures currently scheduled!).
                tokio::task::spawn_blocking(move || {
                    let _parent_guard = parent_span.enter();
                    let mut r = resp?;
                    let _span = tracing::trace_span!(
                        parent: &parent_span,
                        "fetch_chunks::batch_decode",
                        num_chunks = r.chunks.len(),
                    )
                    .entered();

                    // Time the decode loop from inside the blocking task so we
                    // capture true CPU time, not scheduling wait.
                    let decode_start = web_time::Instant::now();
                    payloads.detach(&mut r.chunks);
                    let result: Result<Vec<Chunk>, ApiError> = r
                        .chunks
                        .into_iter()
                        .map(|arrow_msg| decode_chunk(&arrow_msg, &origin, trace_id))
                        .try_collect();
                    if let Some(acc) = &decode_time_us {
                        acc.fetch_add(decode_start.elapsed().as_micros() as u64, Ordering::Relaxed);
                    }
                    result
                })
            }
        })
        .map({
            let origin = origin.clone();
            move |res| {
                res.map_err(|err| {
                    ApiError::internal_with_source(
                        &origin,
                        trace_id,
                        err,
                        "failed to sync on /FetchChunks response stream",
                    )
                })
                .flatten()
            }
        });
    crate::ApiResponseStream::new(origin, stream, trace_id)
}

// This code path happens to be shared between native and web, but we don't have a Tokio runtime on web!
#[cfg(target_arch = "wasm32")]
pub fn fetch_chunks_response_to_chunks(
    response: crate::FetchChunksResponseStream,
    payloads: FetchedPayloads,
    decode_time_us: Option<Arc<std::sync::atomic::AtomicU64>>,
) -> crate::ApiResponseStream<Vec<Chunk>> {
    let origin = response.origin().clone();
    let trace_id = response.trace_id();
    let stream = response.map({
        let origin = origin.clone();
        move |resp| {
            let origin = origin.clone();
            let mut resp = resp?;

            let _span =
                tracing::trace_span!("fetch_chunks::batch_decode", num_chunks = resp.chunks.len())
                    .entered();

            let decode_start = web_time::Instant::now();
            payloads.detach(&mut resp.chunks);
            let result: Result<Vec<Chunk>, ApiError> = resp
                .chunks
                .into_iter()
                .map(|arrow_msg| decode_chunk(&arrow_msg, &origin, trace_id))
                .try_collect();
            if let Some(acc) = &decode_time_us {
                acc.fetch_add(
                    decode_start.elapsed().as_micros() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
            result
        }
    });
    crate::ApiResponseStream::new(origin, stream, trace_id)
}

/// Callback invoked as chunks are downloaded.
///
/// Arguments: `(total_bytes_downloaded, total_bytes_expected)`.
/// `total_bytes_expected` may be `None` if the total size is not known.
pub type ProgressCallback = std::sync::Arc<dyn Fn(u64, Option<u64>) + Send + Sync>;

bitflags::bitflags! {
    /// Which parts of a segment to stream from the server.
    ///
    /// A segment on the server may have an associated default blueprint.
    /// This controls whether we download that blueprint, the segment data, or both.
    ///
    /// Defaults to downloading all parts.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct SegmentDownload: u8 {
        const SEGMENT = 0b1;
        const BLUEPRINT = 0b10;
    }
}

impl Default for SegmentDownload {
    fn default() -> Self {
        Self::all()
    }
}

/// Options that control how segment data is streamed from the server.
#[derive(Clone, Default)]
pub struct StreamingOptions {
    /// If `true`, download all chunks eagerly instead of relying on
    /// on-demand streaming via the RRD manifest.
    ///
    /// This is useful for downloading a full recording to disk.
    pub force_full_download: bool,

    /// Which parts of the segment to download.
    pub download: SegmentDownload,

    /// Optional callback invoked as chunks are downloaded.
    pub on_progress: Option<ProgressCallback>,
}

impl std::fmt::Debug for StreamingOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            force_full_download,
            download,
            on_progress,
        } = self;
        f.debug_struct("StreamingOptions")
            .field("force_full_download", force_full_download)
            .field("download", download)
            .field("on_progress", &on_progress.as_ref().map(|_| "…"))
            .finish()
    }
}

/// The store id of a blueprint streamed from a segment of a blueprint dataset.
///
/// The recording id is the blueprint's segment id, so streaming the same blueprint again uses the
/// same store.
pub fn blueprint_store_id_for_segment(
    application_id: ApplicationId,
    blueprint_segment: &SegmentId,
) -> StoreId {
    StoreId::new(StoreKind::Blueprint, application_id, blueprint_segment)
}

async fn stream_blueprint_segment(
    client: &mut ConnectionClient,
    tx: &re_log_channel::LogSender,
    blueprint_store_id: StoreId,
    blueprint_dataset: EntryId,
    blueprint_segment: SegmentId,
) -> Result<ControlFlow<()>, ApiError> {
    let blueprint_store_info = StoreInfo {
        store_id: blueprint_store_id,
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    // Blueprints are always fully downloaded regardless of recording streaming options.
    if stream_segment_from_server(
        client,
        blueprint_store_info,
        tx,
        blueprint_dataset,
        None,
        blueprint_segment,
        re_uri::Fragment::default(),
        &StreamingOptions::default(),
    )
    .await?
    .is_break()
    {
        Ok(ControlFlow::Break(()))
    } else {
        Ok(ControlFlow::Continue(()))
    }
}

/// Logs a blueprint that failed to load, which its caller then skips.
///
/// Messages already streamed for the blueprint stay in the viewer, but the caller must not
/// register or activate the blueprint.
fn warn_blueprint_skipped(
    err: &ApiError,
    blueprint_dataset: EntryId,
    blueprint_segment: &SegmentId,
) {
    re_log::warn!(
        "{}",
        re_error::format_with_details(
            format!("Failed to load the blueprint, skipping it: {err}"),
            format!(
                "Blueprint dataset id: {blueprint_dataset}\nBlueprint segment id: {blueprint_segment}"
            ),
        )
    );
}

/// Create a log channel for streaming a blueprint from a catalog server.
pub fn blueprint_log_channel(
    origin: re_uri::Origin,
    blueprint_dataset: EntryId,
    blueprint_segment: &SegmentId,
) -> (re_log_channel::LogSender, re_log_channel::LogReceiver) {
    let source_uri = re_uri::DatasetUri {
        origin,
        dataset_id: blueprint_dataset.id,
        resource: re_uri::DatasetResource::Segments,
        segment_id: Some(blueprint_segment.clone()),
        fragment: re_uri::Fragment::default(),
    };

    re_log_channel::log_channel(re_log_channel::LogSource::RedapGrpcStream {
        uri: source_uri,
        open_behavior: re_log_channel::RecordingOpenBehavior::Background,
    })
}

/// Stream a registered `.rbl` blueprint segment into an existing log channel.
///
/// This streams the blueprint data, registers it with the target table, and then sends a successful
/// quit marker on the same channel.
/// It does not emit a [`LogMsg::BlueprintActivationCommand`], so the blueprint is not activated as
/// an application default blueprint.
///
/// Loading is best effort: if the blueprint fails to load, this logs a warning, skips the
/// registration, and still sends the quit marker. Blueprint data streamed before the failure
/// stays in the viewer. It returns early without the quit marker only when the receiver
/// disconnects.
pub async fn stream_table_blueprint_segment_from_server(
    client: ConnectionClient,
    tx: re_log_channel::LogSender,
    blueprint_store_id: StoreId,
    table_ref: re_uri::TableReference,
    blueprint_dataset: EntryId,
    blueprint_segment: SegmentId,
) -> ApiResult {
    let registration = DefaultBlueprintRegistration {
        blueprint_id: blueprint_store_id.clone(),
        target: BlueprintTarget::Table(table_ref),
    };

    stream_blueprint_segment_then_send(
        client,
        tx,
        blueprint_store_id,
        blueprint_dataset,
        blueprint_segment,
        registration.into(),
    )
    .await
}

/// Stream a registered `.rbl` blueprint segment into an existing log channel, and make it the
/// active blueprint of its application.
///
/// The application's default blueprint stays the same.
///
/// Loading is best effort, the same way as for [`stream_table_blueprint_segment_from_server`].
pub async fn stream_active_blueprint_segment_from_server(
    client: ConnectionClient,
    tx: re_log_channel::LogSender,
    blueprint_store_id: StoreId,
    blueprint_dataset: EntryId,
    blueprint_segment: SegmentId,
) -> ApiResult {
    let activation = LogMsg::BlueprintActivationCommand(BlueprintActivationCommand {
        blueprint_id: blueprint_store_id.clone(),
        make_active: true,
        make_default: false,
    });

    stream_blueprint_segment_then_send(
        client,
        tx,
        blueprint_store_id,
        blueprint_dataset,
        blueprint_segment,
        activation.into(),
    )
    .await
}

/// Streams a blueprint segment, sends `on_loaded` once all of it is streamed, and then sends a
/// successful quit marker.
///
/// If the blueprint fails to load, this logs a warning and skips `on_loaded`.
async fn stream_blueprint_segment_then_send(
    mut client: ConnectionClient,
    tx: re_log_channel::LogSender,
    blueprint_store_id: StoreId,
    blueprint_dataset: EntryId,
    blueprint_segment: SegmentId,
    on_loaded: DataSourceMessage,
) -> ApiResult {
    match stream_blueprint_segment(
        &mut client,
        &tx,
        blueprint_store_id,
        blueprint_dataset,
        blueprint_segment.clone(),
    )
    .await
    {
        Ok(ControlFlow::Break(())) => return Ok(()),

        Ok(ControlFlow::Continue(())) => {
            if tx.send(on_loaded).is_err() {
                re_log::debug!("Receiver disconnected");
                return Ok(());
            }
        }

        Err(err) => warn_blueprint_skipped(&err, blueprint_dataset, &blueprint_segment),
    }

    if tx.quit(None).is_err() {
        re_log::debug!("Receiver disconnected");
    }

    Ok(())
}

/// Stream a recording segment from a catalog server, including its registered default blueprint.
///
/// If the dataset has a default blueprint, this first streams that `.rbl` segment and emits a
/// [`LogMsg::BlueprintActivationCommand`] with `make_default = true` for the recording's
/// application id.
/// The blueprint is best effort: if it fails to load, this logs a warning and sends no activation
/// command.
/// It then streams the requested recording segment, whose errors this returns.
///
/// Use [`stream_table_blueprint_segment_from_server`] instead when a registered blueprint should
/// be streamed for a table without changing the active/default blueprint.
pub async fn stream_blueprint_and_segment_from_server(
    mut client: ConnectionClient,
    tx: re_log_channel::LogSender,
    uri: re_uri::DatasetUri,
    options: StreamingOptions,
) -> ApiResult {
    re_log::debug!("Loading {uri}…");

    let (Some(segment_id), Some(recording_store_id)) = (uri.segment_id.clone(), uri.store_id())
    else {
        return Err(ApiError::invalid_arguments(
            client.origin(),
            re_error::format_with_details(
                "Cannot stream a dataset that names no segment",
                format!("Uri: {uri}"),
            ),
        ));
    };

    // The entry tells us which default blueprint to stream, and which kind of dataset this is.
    let dataset_entry = client.read_dataset_entry(uri.dataset_id.into()).await?;

    // An asset is a segment of the dataset's hidden asset dataset, which the url does not name.
    let (dataset_id, dataset_kind) = match uri.resource {
        re_uri::DatasetResource::Segments => (
            EntryId::from(uri.dataset_id),
            dataset_entry.handle.dataset_kind,
        ),

        re_uri::DatasetResource::Assets => {
            let Some(asset_dataset) = dataset_entry.dataset_details.asset_dataset else {
                return Err(ApiError::invalid_arguments(
                    client.origin(),
                    re_error::format_with_details(
                        "No assets are registered for this dataset",
                        format!("Uri: {uri}"),
                    ),
                ));
            };

            (asset_dataset, DatasetKind::Asset)
        }

        re_uri::DatasetResource::Schema => {
            return Err(ApiError::invalid_arguments(
                client.origin(),
                format!("Cannot stream a dataset schema\nUri: {uri}"),
            ));
        }
    };

    // An asset shows something else entirely than the dataset's own segments, so the dataset's
    // blueprint does not apply to it.
    if dataset_kind != DatasetKind::Asset && options.download.contains(SegmentDownload::BLUEPRINT) {
        if let Some((blueprint_dataset, blueprint_segment)) =
            dataset_entry.dataset_details.default_blueprint()
        {
            re_log::debug!("Streaming blueprint dataset {blueprint_dataset}");

            let blueprint_store_id = blueprint_store_id_for_segment(
                recording_store_id.application_id().clone(),
                &blueprint_segment,
            );

            match stream_blueprint_segment(
                &mut client,
                &tx,
                blueprint_store_id.clone(),
                blueprint_dataset,
                blueprint_segment.clone(),
            )
            .await
            {
                Ok(ControlFlow::Break(())) => return Ok(()),

                Ok(ControlFlow::Continue(())) => {
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
                }

                // Without its blueprint the recording still loads, with the viewer's default
                // layout.
                Err(err) => warn_blueprint_skipped(&err, blueprint_dataset, &blueprint_segment),
            }
        } else {
            re_log::debug!("No blueprint dataset found for {uri}");
        }
    }

    let re_uri::DatasetUri {
        origin: _,
        dataset_id: _,
        resource: _,
        segment_id: _, // Taken above
        fragment,
    } = uri;

    if options.download.contains(SegmentDownload::SEGMENT) {
        let store_info = StoreInfo {
            store_id: recording_store_id,
            store_source: StoreSource::Unknown,
            store_version: None,
        };

        // Only recording datasets have assets, so a blueprint or asset dataset has none to look up.
        let asset_dataset = if dataset_kind == DatasetKind::Recording {
            dataset_entry.dataset_details.asset_dataset
        } else {
            None
        };

        if stream_segment_from_server(
            &mut client,
            store_info,
            &tx,
            dataset_id,
            asset_dataset,
            segment_id,
            fragment,
            &options,
        )
        .await?
        .is_break()
        {
            return Ok(());
        }
    }

    Ok(())
}

/// Low-level function to stream data as a chunk store from a server.
async fn stream_segment_from_server(
    client: &mut ConnectionClient,
    store_info: StoreInfo,
    tx: &re_log_channel::LogSender,
    dataset_id: EntryId,
    asset_dataset: Option<EntryId>,
    segment_id: SegmentId,
    fragment: re_uri::Fragment,
    options: &StreamingOptions,
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
                ViewerControlCommand::SetUrlFragment {
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

    if let Some(asset_dataset) = asset_dataset {
        // Only assets that apply to the opened segment are streamed.
        for asset_segment_id in asset_segments(client, asset_dataset, &segment_id).await {
            // TODO(RR-5399): Cache asset manifests?
            // We don't support assets without manifests.
            let streamed = match client
                .get_rrd_manifest_stream(asset_dataset, asset_segment_id.clone())
                .await
            {
                Ok(manifest_stream) => {
                    stream_manifest(
                        client,
                        tx,
                        manifest_stream,
                        options,
                        &store_id,
                        ManifestKind::Asset,
                    )
                    .await
                }
                Err(err) => Err(err),
            };
            match streamed {
                Ok(ManifestOutcome::Loaded) => {}

                Ok(ManifestOutcome::ReceiverDisconnected) => {
                    return Ok(ControlFlow::Break(()));
                }

                Err(err) => {
                    re_log::warn_once!(
                        "{}",
                        re_error::format_with_details(
                            format!("Failed to stream asset manifest, skipping it: {err}"),
                            format!("Asset segment id: {asset_segment_id}"),
                        )
                    );
                }
            }
        }
    }

    // TODO(RR-2976): Do not, under any circumstances, try to chain gRPC streams
    // together. Interlaced streams are a giant footgun that will invariably lead to the exhaustion
    // of client's HTTP2 connection window, and ultimately to a complete stall of the entire system.
    // See the attached issues for more information.

    let manifest_stream = client
        .get_rrd_manifest_stream(dataset_id, segment_id)
        .await?;

    match stream_manifest(
        client,
        tx,
        manifest_stream,
        options,
        &store_id,
        ManifestKind::Segment,
    )
    .await?
    {
        ManifestOutcome::Loaded => Ok(ControlFlow::Continue(())),
        ManifestOutcome::ReceiverDisconnected => Ok(ControlFlow::Break(())),
    }
}

/// Whether a manifest describes a segment's own data, or an asset that segment references.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ManifestKind {
    /// The manifest of the segment being loaded.
    Segment,

    /// The manifest of an asset, which lives in the asset dataset and is shared by many segments.
    Asset,
}

/// What came out of streaming a single manifest.
enum ManifestOutcome {
    /// The manifest was streamed, and any chunks it covers have been handled.
    Loaded,

    /// The log channel receiver went away, so there is nobody left to stream to.
    ReceiverDisconnected,
}

async fn stream_manifest(
    client: &ConnectionClient,
    tx: &re_log_channel::LogSender,
    manifest_stream: crate::ApiResponseStream<RawRrdManifest>,
    options: &StreamingOptions,
    store_id: &StoreId,
    manifest_kind: ManifestKind,
) -> ApiResult<ManifestOutcome> {
    let start_time = web_time::Instant::now();
    let trace_id = manifest_stream.trace_id();
    let mut manifest_stream = std::pin::pin!(manifest_stream);

    let mut raw_rrd_manifest_parts = Vec::new();
    let mut rrd_manifest_parts: Vec<Arc<RrdManifest>> = Vec::new();

    while let Some(part_result) = manifest_stream.next().await {
        let raw_rrd_manifest_part = part_result?;

        let (raw_rrd_manifest_part, rrd_manifest) = if manifest_kind == ManifestKind::Asset {
            asset_manifest(client, raw_rrd_manifest_part).map_err(|err| {
                ApiError::invalid_arguments_with_source(
                    client.origin(),
                    trace_id,
                    err,
                    "Invalid asset manifest part",
                )
            })?
        } else {
            let rrd_manifest = RrdManifest::try_new(&raw_rrd_manifest_part).map_err(|err| {
                ApiError::invalid_arguments_with_source(
                    client.origin(),
                    trace_id,
                    err,
                    "Invalid RRD manifest part",
                )
            })?;

            (raw_rrd_manifest_part, rrd_manifest)
        };

        let part_nr = rrd_manifest_parts.len() + 1;
        re_log::trace!(
            "Received RRD manifest part #{part_nr}/? ({} deflated, {:.1}s elapsed)",
            re_format::format_bytes(raw_rrd_manifest_part.total_size_bytes() as _),
            start_time.elapsed().as_secs_f32(),
        );

        let rrd_manifest = Arc::new(rrd_manifest);

        if tx
            .send(DataSourceMessage::RrdManifest {
                store_id: store_id.clone(),
                manifest: rrd_manifest.clone(),
            })
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(ManifestOutcome::ReceiverDisconnected);
        }

        raw_rrd_manifest_parts.push(raw_rrd_manifest_part);
        rrd_manifest_parts.push(rrd_manifest);
    }

    if rrd_manifest_parts.is_empty() {
        return Err(ApiError::deserialization(
            client.origin(),
            trace_id,
            "failed to parse the response for /GetRrdManifest (no data)",
        ));
    }

    let part_nr = rrd_manifest_parts.len();
    re_log::trace!(
        "Full RRD manifest loaded in {:.1}s in {}",
        start_time.elapsed().as_secs_f32(),
        re_format::format_plural_s(part_nr, "part")
    );

    // An asset manifest is only a part of what the store will receive, so the store is not
    // done until its own manifest has arrived.
    if manifest_kind == ManifestKind::Segment
        && tx
            .send(DataSourceMessage::RrdManifestComplete(store_id.clone()))
            .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(ManifestOutcome::ReceiverDisconnected);
    }

    match store_id.kind() {
        StoreKind::Recording if !options.force_full_download => {
            re_log::trace!("Letting the viewer load chunks on-demand");
            Ok(ManifestOutcome::Loaded)
        }
        StoreKind::Recording | StoreKind::Blueprint => {
            re_log::debug!("Loading all of the chunks in one go; most important first");
            let combined = RawRrdManifest::merge(store_id.clone(), raw_rrd_manifest_parts)
                .map_err(|err| {
                    ApiError::invalid_arguments_with_source(
                        client.origin(),
                        trace_id,
                        err,
                        "Failed to merge RRD manifest parts",
                    )
                })?;
            let combined = RrdManifest::try_new(&combined).map_err(|err| {
                ApiError::invalid_arguments_with_source(
                    client.origin(),
                    trace_id,
                    err,
                    "Invalid merged RRD manifest",
                )
            })?;
            let batch = sort_batch(combined.chunk_fetcher_rb()).map_err(|err| {
                ApiError::invalid_arguments_with_source(
                    client.origin(),
                    trace_id,
                    err,
                    "Failed to sort chunk index",
                )
            })?;

            // TODO(isse): Make sure full download works for assets.
            load_chunks(client, tx, store_id, batch, options)
                .await
                .map(|flow| match flow {
                    ControlFlow::Continue(()) => ManifestOutcome::Loaded,
                    ControlFlow::Break(()) => ManifestOutcome::ReceiverDisconnected,
                })
        }
    }
}

/// Takes a dataframe that looks like an [`RrdManifest`] (has a `chunk_key` column).
#[tracing::instrument(skip_all, fields(
    num_chunks = tracing::field::Empty,
    total_size_bytes = tracing::field::Empty,
    downloaded_bytes = tracing::field::Empty,
))]
async fn load_chunks(
    client: &ConnectionClient,
    tx: &re_log_channel::LogSender,
    store_id: &StoreId,
    full_batch: RecordBatch,
    options: &StreamingOptions,
) -> ApiResult<ControlFlow<()>> {
    let num_chunks = full_batch.num_rows();
    let total_size_bytes = total_size_bytes_from_batch(&full_batch);

    let span = tracing::Span::current();
    span.record("num_chunks", num_chunks);
    if let Some(total_size_bytes) = total_size_bytes {
        span.record("total_size_bytes", total_size_bytes);
    }

    let total_size_str = total_size_bytes
        .map(|bytes| re_format::format_bytes(bytes as _))
        .unwrap_or_else(|| "unknown size".to_owned());
    re_log::debug!(
        "Downloading {} chunks ({}) from server…",
        re_format::format_uint(num_chunks),
        total_size_str,
    );
    if 25_000 < num_chunks {
        re_log::debug_warn!(
            "There are {} chunks in this recording. Consider running `rerun rrd optimize` on it!",
            re_format::format_uint(num_chunks)
        );
    }

    use futures::stream::FuturesUnordered;

    // Batch requests in groups of N=32 rows.
    const BATCH_SIZE: usize = 32;
    let mut futures = FuturesUnordered::new();

    for start in (0..num_chunks).step_by(BATCH_SIZE) {
        let end = usize::min(start + BATCH_SIZE, num_chunks);
        let small_batch = full_batch.slice(start, end - start);

        let mut client = client.clone();
        let tx = tx.clone();
        let store_id = store_id.clone();

        futures.push(async move {
            load_small_chunk_batch(&mut client, &tx, &store_id, &small_batch).await
        });
    }

    let mut downloaded_bytes: u64 = 0;

    while let Some(res) = futures::stream::StreamExt::next(&mut futures).await {
        let (result, batch_bytes) = res?;

        downloaded_bytes += batch_bytes;
        if let Some(on_progress) = &options.on_progress {
            on_progress(downloaded_bytes, total_size_bytes);
        }

        if result.is_break() {
            return Ok(ControlFlow::Break(()));
        }
    }

    span.record("downloaded_bytes", downloaded_bytes);

    re_log::trace!(
        "Finished downloading {} chunks ({}).",
        re_format::format_uint(num_chunks),
        re_format::format_bytes(downloaded_bytes as _),
    );

    Ok(ControlFlow::Continue(()))
}

/// Try to extract total deflated size from the batch's `chunk_byte_size` column.
fn total_size_bytes_from_batch(batch: &RecordBatch) -> Option<u64> {
    let column = RawRrdManifest::COLUMN_CHUNK_BYTE_SIZE.extract(batch).ok()?;
    Some(column.iter().sum())
}

/// Returns `(control_flow, bytes_downloaded)`.
#[tracing::instrument(skip_all, fields(
    num_chunks_in_batch = batch.num_rows(),
    batch_bytes = tracing::field::Empty,
    num_chunks_received = tracing::field::Empty,
))]
async fn load_small_chunk_batch(
    client: &mut ConnectionClient,
    tx: &re_log_channel::LogSender,
    store_id: &StoreId,
    batch: &RecordBatch,
) -> ApiResult<(ControlFlow<()>, u64)> {
    // TODO(RR-3323): FetchChunks should expose a proper bidirectional streaming path on native.
    let chunk_stream = client.fetch_segment_chunks_by_id(batch).await?;
    let mut chunk_stream =
        fetch_chunks_response_to_chunks(chunk_stream, FetchedPayloads::Copied, None);
    let trace_id = chunk_stream.trace_id();

    let mut batch_bytes: u64 = 0;
    let mut num_chunks_received: u64 = 0;

    while let Some(chunks) = chunk_stream.next().await {
        for chunk in chunks? {
            batch_bytes += chunk.heap_size_bytes();
            num_chunks_received += 1;

            if tx
                .send(
                    LogMsg::ArrowMsg(
                        store_id.clone(),
                        // TODO(#10229): this looks to be converting back and forth?
                        chunk.to_arrow_msg().map_err(|err| {
                            ApiError::deserialization_with_source(
                                client.origin(),
                                trace_id,
                                err,
                                "failed to re-encode chunk from /FetchChunks response stream as ArrowMsg",
                            )
                        })?,
                    )
                    .into(),
                )
                .is_err()
            {
                re_log::debug!("Receiver disconnected");
                let span = tracing::Span::current();
                span.record("batch_bytes", batch_bytes);
                span.record("num_chunks_received", num_chunks_received);
                return Ok((ControlFlow::Break(()), batch_bytes));
            }
        }
    }

    let span = tracing::Span::current();
    span.record("batch_bytes", batch_bytes);
    span.record("num_chunks_received", num_chunks_received);

    Ok((ControlFlow::Continue(()), batch_bytes))
}

fn sort_batch(batch: &RecordBatch) -> Result<RecordBatch, ArrowError> {
    // Both columns are guaranteed to exist in the pruned batch:
    let chunk_is_static = RrdManifest::COLUMN_CHUNK_IS_STATIC.extract(batch)?;
    let chunk_id = RrdManifest::COLUMN_CHUNK_ID.extract(batch)?;

    let sort_keys = vec![
        // Static first:
        arrow::compute::SortColumn {
            values: chunk_is_static.into_arrow(),
            options: Some(arrow::compute::SortOptions {
                descending: true,
                nulls_first: true,
            }),
        },
        // Then sort by chunk id (~time)
        arrow::compute::SortColumn {
            values: chunk_id.into_arrow(),
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

#[cfg(test)]
mod tests {
    use re_protos::external::prost::bytes::Bytes;
    use re_protos::log_msg::v1alpha1::ArrowMsg;

    use super::FetchedPayloads;

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn connection_check_uses_only_the_selected_transport() {
        use std::convert::Infallible;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        use http::{Request, Response, StatusCode, header};
        use re_protos::cloud::v1alpha1::rerun_cloud_service_server::SERVICE_NAME;
        use tonic::body::Body;

        use super::{GrpcTransport, send_request};

        for transport in [GrpcTransport::Native, GrpcTransport::Web] {
            let calls = Arc::new(AtomicUsize::new(0));
            let connection = tower::service_fn({
                let calls = calls.clone();
                move |request: Request<Body>| {
                    assert_eq!(
                        request.headers()[header::CONTENT_TYPE],
                        transport.content_type()
                    );
                    calls.fetch_add(1, Ordering::Relaxed);
                    async {
                        Ok::<_, Infallible>(
                            Response::builder()
                                .status(StatusCode::BAD_GATEWAY)
                                .body(Body::empty())
                                .unwrap(),
                        )
                    }
                }
            });
            let request = Request::post(format!("/{SERVICE_NAME}/WhoAmI"))
                .header(header::CONTENT_TYPE, "application/grpc")
                .body(Body::new(http_body_util::Full::new(Bytes::from_static(
                    &[0; 5],
                ))))
                .unwrap();
            let err = send_request(transport, connection, request)
                .await
                .unwrap_err();
            assert_eq!(err.code(), tonic::Code::Unavailable);
            assert_eq!(calls.load(Ordering::Relaxed), 1);
        }
    }

    /// Two payloads sliced out of one shared receive buffer.
    fn response() -> (Bytes, Vec<ArrowMsg>) {
        let buffer = Bytes::from(vec![7_u8; 64]);
        let msgs = [0..32, 32..64]
            .into_iter()
            .map(|range| ArrowMsg {
                payload: buffer.slice(range),
                ..Default::default()
            })
            .collect();
        (buffer, msgs)
    }

    fn aliases(buffer: &Bytes, msg: &ArrowMsg) -> bool {
        buffer.as_ptr_range().contains(&msg.payload.as_ptr())
    }

    #[test]
    fn copied_payloads_stop_aliasing_the_receive_buffer() {
        let (buffer, mut msgs) = response();
        FetchedPayloads::Copied.detach(&mut msgs);
        for msg in &msgs {
            assert!(!aliases(&buffer, msg));
            assert_eq!(&msg.payload[..], &[7_u8; 32]);
        }
    }

    #[test]
    fn shared_payloads_keep_aliasing() {
        let (buffer, mut msgs) = response();
        FetchedPayloads::Shared.detach(&mut msgs);
        assert!(msgs.iter().all(|msg| aliases(&buffer, msg)));
    }
}
