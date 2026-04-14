//! Official gRPC client for the Rerun Data Protocol.

mod api_error;
mod api_response_stream;
mod connection_client;
mod connection_registry;
mod grpc;

pub use self::api_error::{ApiError, ApiErrorKind, ApiResult};
pub use self::api_response_stream::ApiResponseStream;
pub use self::connection_client::{
    FetchChunksResponseStream, GenericConnectionClient, SegmentQueryParams,
};
pub use self::connection_registry::{
    ClientCredentialsError, ConnectionClient, ConnectionRegistry, ConnectionRegistryHandle,
    CredentialSource, Credentials, SourcedCredentials,
};
pub use self::grpc::{
    ChunksWithSegment, RedapClient, StreamingOptions, channel,
    fetch_chunks_response_to_chunk_and_segment_id, stream_blueprint_and_segment_from_server,
};

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

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
        // TODO(emilk): duplicated in `re_grpc_server`
        let status = &self.0;

        write!(f, "gRPC error")?;

        if status.code() != tonic::Code::Unknown {
            write!(f, ", code: '{}'", status.code())?;
        }
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata().as_ref())?;
        }
        Ok(())
    }
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
#[tracing::instrument(skip(f), level = "trace")]
pub async fn with_retry<T, F, Fut>(req_name: &str, f: F) -> ApiResult<T>
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
        let res = f().await;

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
