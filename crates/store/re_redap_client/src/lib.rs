//! Official gRPC client for the Rerun Data Protocol.

mod api_error;
mod api_response_stream;
mod connection_client;
mod connection_registry;
mod grpc;

#[cfg(not(target_arch = "wasm32"))]
mod segment_chunk_provider;

#[cfg(not(target_arch = "wasm32"))]
pub use self::segment_chunk_provider::SegmentChunkProvider;

pub use self::api_error::{ApiError, ApiErrorKind, ApiResult};

pub use self::api_response_stream::ApiResponseStream;
pub use self::connection_client::{
    ConnectionClient, FetchChunksResponseStream, GenericConnectionClient, SegmentQueryParams,
};
pub use self::connection_registry::{
    ClientCredentialsError, ConnectionRegistry, ConnectionRegistryHandle, CredentialSource,
    Credentials, SourcedCredentials,
};
pub use self::grpc::{
    ChunksWithSegment, RedapClient, RedapClientInner, StreamingOptions, channel,
    fetch_chunks_response_to_chunk_and_segment_id, stream_blueprint_and_segment_from_server,
};

/// Re-export of [`opentelemetry::TraceId`] for callers constructing
/// [`ApiError`]s without taking a direct dependency on `opentelemetry`.
pub use opentelemetry::TraceId;

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

/// Per-call deadline for `FetchChunks` requests sent via this client.
///
/// Server-streaming `FetchChunks` calls have no natural cap; without a deadline
/// a stuck stream pegs an HTTP/2 stream slot indefinitely until the caller
/// cancels the surrounding query. Set as the `grpc-timeout` header on every
/// `tonic::Request`, so the server returns `DeadlineExceeded` if a single call
/// exceeds this.
///
/// # Relationship with HTTP/2 keep-alive
///
/// This is **not** the same thing as the HTTP/2 keep-alive on the underlying
/// transport (~50s combined PING interval + ack timeout, configured on both
/// client and server). The two mechanisms are orthogonal:
///
/// - **Keep-alive** is a connection-level liveness probe. It only fires when
///   the connection has been **silent** for a while (no DATA/HEADERS frames),
///   and tears the connection down only if the peer fails to ack a PING. An
///   active `FetchChunks` stream that's emitting chunks keeps the connection
///   non-silent, so keep-alive never fires for it. Its job is to recycle
///   already-dead connections (typically idle TCP killed by a NAT / cloud LB
///   between calls).
///
/// - **This deadline** is a per-call budget. It applies regardless of how
///   chatty the stream is, and bounds the lifetime of an individual call so
///   one runaway server-side handler can't sit on an HTTP/2 stream slot
///   forever.
///
/// Concretely:
///
/// | Situation                                            | Keep-alive (~50s)        | Deadline (this) |
/// |------------------------------------------------------|--------------------------|-----------------|
/// | Healthy stream, frames every few s                   | doesn't fire             | doesn't hit     |
/// | Stream silent for 30s+ but TCP path alive            | PINGs acked, no teardown | runs to deadline → `DeadlineExceeded` |
/// | Stream active but TCP path silently dropped          | teardown at +50s         | moot, conn died first |
/// | Idle between calls, NAT drops TCP                    | teardown at +50s         | not applicable  |
///
/// The value is sized to be a hard cap above observed real `FetchChunks` p95
/// (≈ 250s for large queries on production traffic), so it kills stuck calls
/// without truncating legitimate large fetches.
pub const FETCH_CHUNKS_DEADLINE: std::time::Duration = std::time::Duration::from_secs(300);

/// Responses from the Data Platform can optionally include this header to communicate back the trace id of the request.
const GRPC_RESPONSE_TRACEID_HEADER: &str = "x-request-trace-id";

/// Extract the server's trace-id from gRPC response metadata, if present.
pub fn extract_trace_id(metadata: &tonic::metadata::MetadataMap) -> Option<opentelemetry::TraceId> {
    let s = metadata.get(GRPC_RESPONSE_TRACEID_HEADER)?.to_str().ok()?;
    opentelemetry::TraceId::from_hex(s).ok()
}

/// Wrapper with a nicer error message
#[derive(Debug)]
pub struct TonicStatusError(Box<tonic::Status>);

const _: () = assert!(
    std::mem::size_of::<TonicStatusError>() <= 32,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl AsRef<tonic::Status> for TonicStatusError {
    #[inline]
    fn as_ref(&self) -> &tonic::Status {
        &self.0
    }
}

impl TonicStatusError {
    /// Returns the inner [`tonic::Status`].
    pub fn into_inner(self) -> tonic::Status {
        *self.0
    }
}

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NOTE: duplicated in `re_grpc_server` and `re_grpc_client`
        fmt_tonic_status(f, &self.0)
    }
}

fn fmt_tonic_status(f: &mut std::fmt::Formatter<'_>, status: &tonic::Status) -> std::fmt::Result {
    if status.message().is_empty() {
        write!(f, "gRPC error")?;
    } else {
        write!(f, "{}", status.message())?;
    }

    if status.code() != tonic::Code::Unknown {
        write!(f, " ({})", status.code())?;
    }

    if !status.metadata().is_empty() {
        write!(
            f,
            "{} metadata: {:?}",
            re_error::DETAILS_SEPARATOR,
            status.metadata().as_ref()
        )?;
    }
    Ok(())
}

impl From<tonic::Status> for TonicStatusError {
    fn from(value: tonic::Status) -> Self {
        Self(Box::new(value))
    }
}

impl std::error::Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// Helper function for executing requests or connection attempts with retries.
pub async fn with_retry<T, F, Fut>(req_name: &str, f: F) -> ApiResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ApiResult<T>>,
{
    use tracing::Instrument as _;
    let span = tracing::debug_span!(
        "with_retry",
        otel.name = format!("{req_name} with_retry"),
        req_name,
    );
    with_retry_inner(req_name, f).instrument(span).await
}

async fn with_retry_inner<T, F, Fut>(req_name: &str, f: F) -> ApiResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ApiResult<T>>,
{
    // targeting to have all retries finish under ~5 seconds
    const MAX_ATTEMPTS: usize = 5;

    // 100 200 400 800 1600
    let mut backoff_gen = re_backoff::BackoffGenerator::new(
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(3),
    )
    .expect("base is less than max");

    let mut attempts = 1;
    let mut last_retryable_err = None;

    while attempts <= MAX_ATTEMPTS {
        use tracing::Instrument as _;
        let res = f()
            .instrument(tracing::debug_span!("attempt", attempts))
            .await;

        match res {
            Err(err) if err.kind.is_retryable() => {
                last_retryable_err = Some(err);
                let backoff = backoff_gen.gen_next();

                tracing::trace!(
                    attempts,
                    max_attempts = MAX_ATTEMPTS,
                    ?backoff,
                    "{req_name} failed with retryable gRPC error, retrying after backoff"
                );

                backoff.sleep().await;
            }
            Err(err) => {
                // logging at the trace level to avoid having these spam in debug builds of the viewer
                tracing::trace!(
                    attempts,
                    "{req_name} failed with non-retryable error: {err}"
                );
                return Err(err);
            }

            Ok(value) => {
                tracing::trace!(attempts, "{req_name} succeeded");
                return Ok(value);
            }
        }

        attempts += 1;
    }

    tracing::trace!(
        attempts,
        max_attempts = MAX_ATTEMPTS,
        "{req_name} failed after max retries, giving up"
    );

    Err(last_retryable_err.expect("bug: this should not be None if we reach here"))
}
