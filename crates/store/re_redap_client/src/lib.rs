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
    ChunksWithSegment, RedapClient, RedapClientInner, SegmentDownload, StreamingOptions, channel,
    fetch_chunks_response_to_chunk_and_segment_id, stream_blueprint_and_segment_from_server,
    stream_table_blueprint_segment_from_server, table_blueprint_log_channel,
};

#[cfg(not(target_arch = "wasm32"))]
pub use self::grpc::PoolChannel;

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

/// Responses from the catalog server can optionally include this header to communicate back the trace id of the request.
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

/// Backoff schedule + caps + which errors to retry, for [`retry_loop`].
struct RetryPolicy {
    /// First backoff duration; doubles each attempt up to `max`.
    base: std::time::Duration,

    /// Maximum single backoff duration.
    max: std::time::Duration,

    /// Hard cap on the number of attempts.
    max_attempts: usize,

    /// Optional hard wall-clock budget across all attempts (including sleeps).
    ///
    /// When the next sleep would push the total elapsed time past this budget, we stop and return
    /// the last error instead of sleeping. Bounds total client-side latency regardless of
    /// `max_attempts`.
    total_budget: Option<std::time::Duration>,
}

/// The retry profile used for connection establishment and other broadly-transient failures.
///
/// `~5s` worth of attempts, retrying any [`ApiErrorKind::is_retryable`] error.
const CONNECTION_RETRY_POLICY: RetryPolicy = RetryPolicy {
    base: std::time::Duration::from_millis(100),
    max: std::time::Duration::from_secs(3),
    max_attempts: 5,
    total_budget: None,
};

/// The retry profile used for fail-fast `ResourceExhausted` rejections from server-side admission
/// control (`ScanSegmentTable`, `QueryDataset`).
///
/// These rejections happen at stream-open before any server-side work, so retrying is idempotent.
/// The capacity limiter can stay saturated for the duration of other in-flight scans, so we poll
/// for up to a hard `10s` budget (full-jitter bases `0.25/0.5/1/2/2…s`); `max_attempts` is set high
/// enough that the budget is the binding limit.
const RESOURCE_EXHAUSTED_RETRY_POLICY: RetryPolicy = RetryPolicy {
    base: std::time::Duration::from_millis(250),
    max: std::time::Duration::from_secs(2),
    max_attempts: 12,
    total_budget: Some(std::time::Duration::from_secs(10)),
};

/// Execute requests or connection attempts with retries, using the existing
/// `CONNECTION_RETRY_POLICY` (retries any [`ApiErrorKind::is_retryable`] error).
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
    retry_loop(
        req_name,
        &CONNECTION_RETRY_POLICY,
        |err| err.kind.is_retryable(),
        f,
    )
    .instrument(span)
    .await
}

/// Execute an idempotent, fail-fast read with retries that fire **only** on
/// [`ApiErrorKind::ResourcesExhausted`], using `RESOURCE_EXHAUSTED_RETRY_POLICY`.
///
/// Use this around the *opening* of a gRPC call that the server may reject with
/// `ResourceExhausted` from admission control (never around the consumption of an already-open
/// stream). Other error kinds (including the catch-all `Internal`, which a caller cancellation can
/// surface as) are returned immediately rather than retried.
pub async fn with_retry_resource_exhausted<T, F, Fut>(req_name: &str, f: F) -> ApiResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ApiResult<T>>,
{
    use tracing::Instrument as _;
    let span = tracing::debug_span!(
        "with_retry_resource_exhausted",
        otel.name = format!("{req_name} with_retry_resource_exhausted"),
        req_name,
    );
    retry_loop(
        req_name,
        &RESOURCE_EXHAUSTED_RETRY_POLICY,
        |err| err.kind == ApiErrorKind::ResourcesExhausted,
        f,
    )
    .instrument(span)
    .await
}

async fn retry_loop<T, F, Fut, R>(
    req_name: &str,
    policy: &RetryPolicy,
    should_retry: R,
    f: F,
) -> ApiResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ApiResult<T>>,
    R: Fn(&ApiError) -> bool,
{
    let start = web_time::Instant::now();
    let mut backoff_gen =
        re_backoff::BackoffGenerator::new(policy.base, policy.max).expect("base is less than max");

    let mut attempts = 1;
    let mut last_retryable_err = None;

    while attempts <= policy.max_attempts {
        use tracing::Instrument as _;
        let res = f()
            .instrument(tracing::debug_span!("attempt", attempts))
            .await;

        match res {
            Err(err) if should_retry(&err) => {
                last_retryable_err = Some(err);

                // Backoff happens *between* attempts: on the final attempt there's nothing left to
                // wait for, so give up now instead of paying a sleep we'll never use.
                if attempts >= policy.max_attempts {
                    break;
                }

                let backoff = backoff_gen.gen_next();

                // Respect the wall-clock budget: if sleeping would push us past it, give up now
                // rather than waiting only to time out. The post-loop log reports the give-up.
                if let Some(budget) = policy.total_budget
                    && start.elapsed() + backoff.jittered() > budget
                {
                    break;
                }

                tracing::trace!(
                    attempts,
                    max_attempts = policy.max_attempts,
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

    // A retry give-up can absorb meaningful wall-clock time (up to the policy's budget) before the
    // error surfaces, so log it at `debug` (with elapsed) to aid triage of "why was this slow?".
    // The error itself is still returned to — and surfaced by — the caller.
    tracing::debug!(
        attempts,
        max_attempts = policy.max_attempts,
        elapsed_ms = start.elapsed().as_millis() as u64,
        "{req_name} giving up after exhausting retries"
    );

    Err(last_retryable_err.expect("bug: this should not be None if we reach here"))
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Build an `ApiError` of kind `ResourcesExhausted`, exactly as the client would from a
    /// server `tonic::Status::resource_exhausted`.
    fn resource_exhausted_err() -> ApiError {
        ApiError::tonic(tonic::Status::resource_exhausted("busy"), "test")
    }

    /// A fast policy (sub-ms sleeps) so the retry-logic tests don't actually wait. The public
    /// `with_retry_resource_exhausted` uses the real 250ms→2s profile; the loop behavior is what
    /// we exercise here.
    const FAST_POLICY: RetryPolicy = RetryPolicy {
        base: std::time::Duration::from_millis(1),
        max: std::time::Duration::from_millis(1),
        max_attempts: 12,
        total_budget: None,
    };

    fn is_resource_exhausted(err: &ApiError) -> bool {
        err.kind == ApiErrorKind::ResourcesExhausted
    }

    #[tokio::test]
    async fn resource_exhausted_retries_until_success() {
        let calls = AtomicUsize::new(0);
        let res: ApiResult<u32> =
            retry_loop("test", &FAST_POLICY, is_resource_exhausted, || async {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n < 3 {
                    Err(resource_exhausted_err())
                } else {
                    Ok(42)
                }
            })
            .await;

        assert_eq!(res.expect("should eventually succeed"), 42);
        // 3 failed attempts + 1 success.
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn non_resource_exhausted_is_not_retried() {
        // `Internal` is the catch-all kind a cancellation/unknown status maps to. It must NOT be
        // retried by the resource-exhausted profile. Tested through the public wrapper (the failing
        // attempt returns immediately, so no real backoff is incurred).
        let calls = AtomicUsize::new(0);
        let res: ApiResult<u32> = with_retry_resource_exhausted("test", || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(ApiError::internal("boom"))
        })
        .await;

        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_loop_respects_wall_clock_budget() {
        // Tiny budget so the test runs in tens of ms while still proving the budget (not the
        // attempt cap) is what stops the loop.
        let policy = RetryPolicy {
            base: std::time::Duration::from_millis(10),
            max: std::time::Duration::from_millis(10),
            max_attempts: 1000,
            total_budget: Some(std::time::Duration::from_millis(40)),
        };
        let calls = AtomicUsize::new(0);
        let res: ApiResult<u32> = retry_loop(
            "test",
            &policy,
            |err| err.kind == ApiErrorKind::ResourcesExhausted,
            || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(resource_exhausted_err())
            },
        )
        .await;

        assert!(res.is_err());
        let n = calls.load(Ordering::SeqCst);
        assert!(n >= 2, "should retry at least once, got {n}");
        assert!(
            n < policy.max_attempts,
            "should stop on the wall-clock budget, not the attempt cap, got {n}"
        );
    }
}
