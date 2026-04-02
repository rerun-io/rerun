//! Official gRPC client for the Rerun Data Protocol.

mod connection_client;
mod connection_registry;
mod grpc;

pub use self::connection_client::{
    FetchChunksResponseStream, GenericConnectionClient, ResponseStream, SegmentQueryParams,
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

#[derive(Debug)]
pub struct ApiError {
    /// A message that does NOT include the contents of [`Self::source`].
    pub message: String,

    pub kind: ApiErrorKind,

    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,

    /// When the error comes from the server returning a trace id, we include it in the client
    /// error for easier reporting.
    trace_id: Option<opentelemetry::TraceId>,
}

/// Convenience for `Result<T, ApiError>`
pub type ApiResult<T = ()> = Result<T, ApiError>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ApiErrorKind {
    NotFound,
    AlreadyExists,
    PermissionDenied,
    Unauthenticated,

    /// The gRPC endpoint has not been implemented.
    Unimplemented,
    Connection,
    Timeout,
    Internal,
    InvalidArguments,
    ResourcesExhausted,

    /// Failed to decode data received from the server (e.g. protobuf → Arrow conversion).
    Deserialization,

    /// Failed to encode data for sending to the server.
    Serialization,

    InvalidServer,
}

impl From<tonic::Code> for ApiErrorKind {
    fn from(code: tonic::Code) -> Self {
        match code {
            tonic::Code::NotFound => Self::NotFound,
            tonic::Code::AlreadyExists => Self::AlreadyExists,
            tonic::Code::PermissionDenied => Self::PermissionDenied,
            tonic::Code::ResourceExhausted => Self::ResourcesExhausted,
            tonic::Code::Unauthenticated => Self::Unauthenticated,
            tonic::Code::Unimplemented => Self::Unimplemented,
            tonic::Code::Unavailable => Self::Connection,
            tonic::Code::InvalidArgument => Self::InvalidArguments,
            tonic::Code::DeadlineExceeded => Self::Timeout,
            _ => Self::Internal,
        }
    }
}

impl ApiErrorKind {
    /// Transient errors that may succeed on retry (with backoff).
    pub fn is_retryable(self) -> bool {
        match self {
            Self::Connection | Self::Timeout | Self::Internal | Self::ResourcesExhausted => true,

            Self::NotFound
            | Self::AlreadyExists
            | Self::PermissionDenied
            | Self::Unauthenticated
            | Self::Unimplemented
            | Self::InvalidArguments
            | Self::Deserialization
            | Self::Serialization
            | Self::InvalidServer => false,
        }
    }
}

impl std::fmt::Display for ApiErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "NotFound"),
            Self::AlreadyExists => write!(f, "AlreadyExists"),
            Self::PermissionDenied => write!(f, "PermissionDenied"),
            Self::Unauthenticated => write!(f, "Unauthenticated"),
            Self::Unimplemented => write!(f, "Unimplemented"),
            Self::Connection => write!(f, "Connection"),
            Self::Internal => write!(f, "Internal"),
            Self::InvalidArguments => write!(f, "InvalidArguments"),
            Self::ResourcesExhausted => write!(f, "ResourcesExhausted"),
            Self::Deserialization => write!(f, "Deserialization"),
            Self::Serialization => write!(f, "Serialization"),
            Self::Timeout => write!(f, "Timeout"),
            Self::InvalidServer => write!(f, "InvalidServer"),
        }
    }
}

impl ApiError {
    #[inline]
    fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind,
            source: None,
            trace_id: None,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    #[inline]
    fn new_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        kind: ApiErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind,
            source: Some(Box::new(err)),
            trace_id: None,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    #[inline]
    fn new_with_source_and_trace_id(
        err: impl std::error::Error + Send + Sync + 'static,
        kind: ApiErrorKind,
        message: impl Into<String>,
        trace_id: opentelemetry::TraceId,
    ) -> Self {
        Self {
            message: message.into(),
            kind,
            source: Some(Box::new(err)),
            trace_id: Some(trace_id),
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn tonic(err: tonic::Status, message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = ApiErrorKind::from(err.code());
        let trace_id = extract_trace_id(err.metadata());
        if let Some(trace_id) = trace_id {
            Self::new_with_source_and_trace_id(err, kind, message, trace_id)
        } else {
            Self::new_with_source(err, kind, message)
        }
    }

    /// Failed to decode data received from the server.
    pub fn deserialization(
        trace_id: Option<opentelemetry::TraceId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::Deserialization,
            source: None,
            trace_id,
        }
    }

    /// Failed to decode data received from the server.
    ///
    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn deserialization_with_source(
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::Deserialization,
            source: Some(Box::new(err)),
            trace_id,
        }
    }

    /// Failed to encode data for sending to the server.
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Serialization, message)
    }

    /// Failed to encode data for sending to the server.
    ///
    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn serialization_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::Serialization, message)
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn invalid_arguments_with_source(
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::InvalidArguments,
            source: Some(Box::new(err)),
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn internal_with_source(
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::Internal,
            source: Some(Box::new(err)),
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn connection_with_source(
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::Connection,
            source: Some(Box::new(err)),
            trace_id,
        }
    }

    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Connection, message)
    }

    pub fn permission_denied(
        trace_id: Option<opentelemetry::TraceId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::PermissionDenied,
            source: None,
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn credentials_with_source(
        trace_id: Option<opentelemetry::TraceId>,
        err: ClientCredentialsError,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind: ApiErrorKind::Unauthenticated,
            source: Some(Box::new(err)),
            trace_id,
        }
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn invalid_server(origin: re_uri::Origin, hint: Option<&str>) -> Self {
        let mut msg = format!("{origin} is not a valid Rerun server");
        if let Some(hint) = hint {
            msg.push_str(". ");
            msg.push_str(hint);
        }
        Self::new(ApiErrorKind::InvalidServer, msg)
    }

    /// Helper method to downcast the source error to a `ClientCredentialsError` if possible.
    #[inline]
    pub fn as_client_credentials_error(&self) -> Option<&ClientCredentialsError> {
        self.source
            .as_deref()?
            .downcast_ref::<ClientCredentialsError>()
    }

    #[inline]
    pub fn is_client_credentials_error(&self) -> bool {
        self.kind == ApiErrorKind::Unauthenticated
            && matches!(self.source.as_deref(), Some(e) if e.is::<ClientCredentialsError>())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            message,
            kind,
            source,
            trace_id,
        } = self;

        write!(f, "{message} ({kind})")?;

        if let Some(trace_id) = trace_id {
            write!(f, " (trace-id: {trace_id})")?;
        }

        if let Some(err) = source {
            write!(f, ", {err}")?;
        }

        Ok(())
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
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
