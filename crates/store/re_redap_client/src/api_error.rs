use std::sync::Arc;

use crate::connection_registry::ClientCredentialsError;
use crate::extract_trace_id;

#[derive(Clone, Debug)]
pub struct ApiError {
    /// A message that does NOT include the contents of [`Self::source`].
    pub message: String,

    pub kind: ApiErrorKind,

    pub source: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,

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
            source: Some(Arc::new(err)),
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
            source: Some(Arc::new(err)),
            trace_id: Some(trace_id),
        }
    }

    /// Construct an [`ApiError`] with an explicit `kind` and an optional `trace_id`.
    ///
    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn with_kind_and_source(
        kind: ApiErrorKind,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            kind,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn tonic(err: tonic::Status, message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = ApiErrorKind::from(err.code());
        let trace_id = extract_trace_id(err.metadata());
        let err = crate::TonicStatusError::from(err); // Wrap in TonicStatusError so we get our nice Display formatting
        if let Some(trace_id) = trace_id {
            Self::new_with_source_and_trace_id(err, kind, message, trace_id)
        } else {
            Self::new_with_source(err, kind, message)
        }
    }

    /// Sets the trace-id if not already set.
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: Option<opentelemetry::TraceId>) -> Self {
        if self.trace_id.is_none() {
            self.trace_id = trace_id;
        }
        self
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
            source: Some(Arc::new(err)),
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
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Internal, message)
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
            source: Some(Arc::new(err)),
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
            source: Some(Arc::new(err)),
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
            source: Some(Arc::new(err)),
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
            .as_ref()?
            .downcast_ref::<ClientCredentialsError>()
    }

    #[inline]
    pub fn is_client_credentials_error(&self) -> bool {
        self.kind == ApiErrorKind::Unauthenticated
            && matches!(self.source.as_ref(), Some(e) if e.is::<ClientCredentialsError>())
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
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}
