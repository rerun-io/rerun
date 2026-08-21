use std::sync::Arc;

use crate::connection_registry::ClientCredentialsError;
use crate::extract_trace_id;

/// Something went wrong while talking to a server.
///
/// Every [`ApiError`] names the server it is about, because the viewer can be connected to several
/// at once. It is therefore for server interactions only: a failure with no server behind it (a
/// local decode, a bad argument the client caught on its own) should use its own error type.
///
/// [`std::fmt::Display`] renders it as `{message}: {source} ({kind})`, followed by a details
/// section with the server, the trace-id, and whatever details the source carried. Keep `message`
/// free of the source's text: it is added when displaying.
#[derive(Clone, Debug)]
pub struct ApiError {
    /// The server this error is about.
    ///
    /// The viewer can be connected to several servers at once, so an error that doesn't name one
    /// leaves the user guessing.
    pub origin: re_uri::Origin,

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
    FailedPrecondition,
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
            tonic::Code::FailedPrecondition => Self::FailedPrecondition,
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
            | Self::FailedPrecondition
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
            Self::FailedPrecondition => write!(f, "FailedPrecondition"),
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
    fn new(origin: &re_uri::Origin, kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind,
            source: None,
            trace_id: None,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    #[inline]
    fn new_with_source(
        origin: &re_uri::Origin,
        err: impl std::error::Error + Send + Sync + 'static,
        kind: ApiErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind,
            source: Some(Arc::new(err)),
            trace_id: None,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    #[inline]
    fn new_with_source_and_trace_id(
        origin: &re_uri::Origin,
        err: impl std::error::Error + Send + Sync + 'static,
        kind: ApiErrorKind,
        message: impl Into<String>,
        trace_id: opentelemetry::TraceId,
    ) -> Self {
        Self {
            origin: origin.clone(),
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
        origin: &re_uri::Origin,
        kind: ApiErrorKind,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Convert an unsuccessful HTTP status into an [`ApiError`].
    ///
    /// Authentication, authorization, missing-resource, precondition, and throttling responses map
    /// to their corresponding API error kinds.
    /// Server errors are treated as connection failures so callers may retry them.
    /// Other statuses indicate that the server did not honor the expected HTTP protocol.
    pub fn http_status(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        status: u16,
        message: impl Into<String>,
    ) -> Self {
        Self::http_status_with_source(
            origin,
            trace_id,
            status,
            std::io::Error::other(format!("HTTP {status}")),
            message,
        )
    }

    /// Convert an unsuccessful HTTP status into an [`ApiError`] with a specific source error.
    ///
    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn http_status_with_source(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        status: u16,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        let kind = match status {
            401 => ApiErrorKind::Unauthenticated,
            403 => ApiErrorKind::PermissionDenied,
            404 => ApiErrorKind::NotFound,
            412 => ApiErrorKind::FailedPrecondition,
            429 => ApiErrorKind::ResourcesExhausted,
            500..=599 => ApiErrorKind::Connection,
            _ => ApiErrorKind::InvalidServer,
        };
        Self::with_kind_and_source(origin, kind, trace_id, err, message)
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn tonic(origin: &re_uri::Origin, err: tonic::Status, message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = ApiErrorKind::from(err.code());

        // On the web, the browser blocks failed `fetch` calls (CORS, mixed content, server
        // unreachable, DNS, …) and — for security reasons — hides the actual cause from
        // JavaScript, surfacing only an opaque message (e.g. `TypeError: Failed to fetch` in
        // Chrome, `NetworkError when attempting to fetch resource` in Firefox, `Load failed` in
        // Safari). `tonic-web-wasm-client` wraps all of these as `Error::JsError`, which tonic
        // turns into a `Code::Unknown` status whose message is prefixed `js api error:`.
        //
        // Note: other `Code::Unknown` variants (malformed response, missing content-type, …)
        // mean the server *did* respond but with non-gRPC data (wrong port, a proxy serving
        // HTML, …) — those are not network failures, so we deliberately don't add the hint there.
        //
        // Point the user at the developer console, where the browser *does* print the real
        // reason (e.g. the missing CORS header).
        #[cfg(target_arch = "wasm32")]
        let (kind, message) = if err.code() == tonic::Code::Unknown
            && err.message().to_ascii_lowercase().contains("js api error")
        {
            (
                ApiErrorKind::Connection,
                format!(
                    "{message}: failed to reach the server. \
                     This is often a CORS issue, but can also mean the server is unreachable. \
                     Open your browser's developer console for the underlying error."
                ),
            )
        } else {
            (kind, message)
        };

        let trace_id = extract_trace_id(err.metadata());
        let err = crate::TonicStatusError::from(err); // Wrap in TonicStatusError so we get our nice Display formatting
        if let Some(trace_id) = trace_id {
            Self::new_with_source_and_trace_id(origin, err, kind, message, trace_id)
        } else {
            Self::new_with_source(origin, err, kind, message)
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
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
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
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::Deserialization,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Failed to decode a quiver record batch received from the server.
    ///
    /// Decoding server data is a [`ApiErrorKind::Deserialization`]; the quiver error names the
    /// offending column and the exact mismatch, so no extra message is needed.
    pub fn deserialization_quiver(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: quiver::Error,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: "failed to decode record batch".to_owned(),
            kind: ApiErrorKind::Deserialization,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Like [`Self::deserialization_quiver`], but names where the batch came from (the endpoint or
    /// response stream); the quiver error itself only describes the schema mismatch.
    pub fn deserialization_quiver_from(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: quiver::Error,
        context: impl std::fmt::Display,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: format!("failed to decode record batch from {context}"),
            kind: ApiErrorKind::Deserialization,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Failed to encode data for sending to the server.
    pub fn serialization(origin: &re_uri::Origin, message: impl Into<String>) -> Self {
        Self::new(origin, ApiErrorKind::Serialization, message)
    }

    /// Failed to encode a quiver record batch for sending to the server.
    pub fn serialization_quiver(origin: &re_uri::Origin, err: quiver::Error) -> Self {
        Self::new_with_source(
            origin,
            err,
            ApiErrorKind::Serialization,
            "failed to encode record batch",
        )
    }

    /// Failed to encode data for sending to the server.
    ///
    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn serialization_with_source(
        origin: &re_uri::Origin,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self::new_with_source(origin, err, ApiErrorKind::Serialization, message)
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn invalid_arguments_with_source(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::InvalidArguments,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    pub fn invalid_arguments(origin: &re_uri::Origin, message: impl Into<String>) -> Self {
        Self::new(origin, ApiErrorKind::InvalidArguments, message)
    }

    pub fn internal(origin: &re_uri::Origin, message: impl Into<String>) -> Self {
        Self::new(origin, ApiErrorKind::Internal, message)
    }

    /// Failed to decode a quiver record batch. The quiver error names the offending column and the
    /// record-batch schema, so no extra message is needed.
    pub fn internal_quiver(origin: &re_uri::Origin, err: quiver::Error) -> Self {
        Self::new_with_source(
            origin,
            err,
            ApiErrorKind::Internal,
            "failed to decode record batch",
        )
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn internal_with_source(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::Internal,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn connection_with_source(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: impl std::error::Error + Send + Sync + 'static,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::Connection,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    pub fn connection(origin: &re_uri::Origin, message: impl Into<String>) -> Self {
        Self::new(origin, ApiErrorKind::Connection, message)
    }

    pub fn permission_denied(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::PermissionDenied,
            source: None,
            trace_id,
        }
    }

    /// Do NOT include `err` in the `message` - it will be added for you.
    pub fn credentials_with_source(
        origin: &re_uri::Origin,
        trace_id: Option<opentelemetry::TraceId>,
        err: ClientCredentialsError,
        message: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.clone(),
            message: message.into(),
            kind: ApiErrorKind::Unauthenticated,
            source: Some(Arc::new(err)),
            trace_id,
        }
    }

    /// Raised when `GET /version` against the requested origin returns a non-2xx response.
    ///
    /// The included status line and body snippet usually tell the user whether the path is
    /// wrong (404 from a non-Rerun HTTP server), the server is down (5xx), or they hit a
    /// reverse proxy that redirected somewhere unexpected. Connection-refused (wrong port
    /// or server not running) hits a different error path above.
    pub fn invalid_server_with_response(
        origin: &re_uri::Origin,
        status: u16,
        status_text: &str,
        body_snippet: Option<&str>,
        hint: Option<&str>,
    ) -> Self {
        let mut msg =
            format!("not a valid Rerun server (GET /version returned HTTP {status} {status_text})");
        if let Some(body) = body_snippet.filter(|s| !s.is_empty()) {
            msg.push_str(": ");
            msg.push_str(body);
        }
        if let Some(hint) = hint {
            msg.push_str(". ");
            msg.push_str(hint);
        }
        Self::new(origin, ApiErrorKind::InvalidServer, msg)
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
            origin,
            trace_id,
        } = self;

        let source = source.as_ref().map(|err| err.to_string());

        let mut details = Vec::new();

        details.push(format!("Server: {origin}"));

        if let Some(trace_id) = trace_id {
            details.push(format!("trace-id: {trace_id}"));
        }

        let source_summary = source.as_deref().map(|source| {
            let source = re_error::StructuredError::parse(source);
            details.extend(source.details);
            source.summary
        });

        // A gRPC source already names its status code, and our kind is derived from that very
        // code; naming it again would say the same thing twice. `Unknown` is the exception: the
        // source leaves that one out, so the kind is all the reader gets.
        let kind_is_the_grpc_code = self
            .source
            .as_ref()
            .and_then(|source| source.downcast_ref::<crate::TonicStatusError>())
            .map(|status| status.as_ref().code())
            .is_some_and(|code| code != tonic::Code::Unknown && ApiErrorKind::from(code) == *kind);

        let kind = if kind_is_the_grpc_code {
            String::new()
        } else {
            format!(" ({kind})")
        };

        let summary = match source_summary {
            Some(source_summary) => format!("{message}: {source_summary}{kind}"),
            None => format!("{message}{kind}"),
        };

        let error = re_error::StructuredError::from_summary(summary).with_details(details);

        write!(f, "{error}")
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worst case: a server error whose message carries details of its own, plus response
    /// metadata, plus a trace-id, plus a known server.
    ///
    /// Everything the user needs must be in the summary, and everything else on the detail lines,
    /// without any of it being said twice.
    #[test]
    fn test_display_of_server_error() {
        let mut status = tonic::Status::not_found(
            "the dataset has no promoted revision yet\n- dataset url: file:///path/to/file",
        );
        status.metadata_mut().insert(
            crate::GRPC_RESPONSE_TRACEID_HEADER,
            tonic::metadata::MetadataValue::from_static("abba000000000000000000000000abba"),
        );

        let origin = "rerun+https://api.example.com:443"
            .parse::<re_uri::Origin>()
            .expect("hardcoded origin should parse");

        let err = ApiError::tonic(&origin, status, "/GetRrdManifest failed");

        assert_eq!(
            err.to_string(),
            "/GetRrdManifest failed: the dataset has no promoted revision yet (NotFound)\n\
             - Server: rerun://api.example.com:443\n\
             - trace-id: abba000000000000000000000000abba\n\
             - dataset url: file:///path/to/file\n\
             - metadata: {\"x-request-trace-id\": \"abba000000000000000000000000abba\"}"
        );
    }

    /// A code that has no `ApiErrorKind` of its own must still be named, or it is lost: several
    /// of them collapse into `Internal`.
    #[test]
    fn test_display_keeps_a_collapsed_grpc_code() {
        let err = ApiError::tonic(
            &re_uri::Origin::test(),
            tonic::Status::aborted("transaction aborted"),
            "/RegisterWithDataset failed",
        );

        assert_eq!(
            err.to_string(),
            "/RegisterWithDataset failed: transaction aborted (Aborted)\n\
             - Server: rerun://example.com:443"
        );
    }

    #[test]
    fn test_display_without_source_or_trace_id() {
        assert_eq!(
            ApiError::internal(&re_uri::Origin::test(), "something went wrong").to_string(),
            "something went wrong (Internal)\n- Server: rerun://example.com:443"
        );
    }
}
