//! Official gRPC client for the Rerun Data Protocol.

mod connection_client;
mod connection_registry;
mod grpc;

pub use self::connection_client::{GenericConnectionClient, SegmentQueryParams};
pub use self::connection_registry::{
    ClientCredentialsError, ConnectionClient, ConnectionRegistry, ConnectionRegistryHandle,
    CredentialSource, Credentials, SourcedCredentials,
};
pub use self::grpc::{
    RedapClient, channel, fetch_chunks_response_to_chunk_and_segment_id,
    stream_blueprint_and_segment_from_server,
};

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

/// Responses from the Data Platform can optionally include this header to communicate back the trace id of the request.
const GRPC_RESPONSE_TRACEID_HEADER: &str = "x-request-trace-id";

/// Controls how to load chunks from the remote server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StreamMode {
    /// Load all data into memory.
    #[default]
    FullLoad,

    /// Larger-than-RAM support.
    ///
    /// Load chunks as needed.
    /// Will start by loading the RRD manifest.
    OnDemand,
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
    pub message: String,
    pub kind: ApiErrorKind,
    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    // when the error comes from the server returning a trace id, we include it in the client
    // error for easier reporting.
    trace_id: Option<String>,
}

/// Convenience for `Result<T, ApiError>`
pub type ApiResult<T = ()> = Result<T, ApiError>;

#[derive(Debug, PartialEq, Eq)]
pub enum ApiErrorKind {
    NotFound,
    AlreadyExists,
    PermissionDenied,
    Unauthenticated,

    /// The gRPC endpoint has not been implemented
    Unimplemented,
    Connection,
    Timeout,
    Internal,
    InvalidArguments,
    Serialization,
    InvalidServer,
}

impl From<tonic::Code> for ApiErrorKind {
    fn from(code: tonic::Code) -> Self {
        match code {
            tonic::Code::NotFound => Self::NotFound,
            tonic::Code::AlreadyExists => Self::AlreadyExists,
            tonic::Code::PermissionDenied => Self::PermissionDenied,
            tonic::Code::Unauthenticated => Self::Unauthenticated,
            tonic::Code::Unimplemented => Self::Unimplemented,
            tonic::Code::Unavailable => Self::Connection,
            tonic::Code::InvalidArgument => Self::InvalidArguments,
            tonic::Code::DeadlineExceeded => Self::Timeout,
            _ => Self::Internal,
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

    #[inline]
    fn new_with_source_and_trace(
        err: impl std::error::Error + Send + Sync + 'static,
        kind: ApiErrorKind,
        message: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind,
            source: Some(Box::new(err)),
            trace_id: Some(trace_id.into()),
        }
    }

    pub fn tonic(err: tonic::Status, message: impl Into<String>) -> Self {
        let message = format!("{}: {}", message.into(), err.message());
        let kind = ApiErrorKind::from(err.code());
        let trace_id = err
            .metadata()
            .get(GRPC_RESPONSE_TRACEID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());
        if let Some(trace_id) = trace_id {
            Self::new_with_source_and_trace(err, kind, message, trace_id)
        } else {
            Self::new_with_source(err, kind, message)
        }
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Serialization, message)
    }

    pub fn serialization_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::Serialization, message)
    }

    pub fn invalid_arguments_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::InvalidArguments, message)
    }

    pub fn internal_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::Internal, message)
    }

    pub fn connection_with_source(
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::Connection, message)
    }

    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Connection, message)
    }

    pub fn credentials_with_source(
        err: ClientCredentialsError,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(err, ApiErrorKind::Unauthenticated, message)
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn invalid_server(origin: re_uri::Origin) -> Self {
        Self::new(
            ApiErrorKind::InvalidServer,
            format!("{origin} is not a valid Rerun server"),
        )
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
        write!(f, "{}", self.message)?;
        if let Some(ref trace_id) = self.trace_id {
            write!(f, " (trace-id: {trace_id})")?;
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
