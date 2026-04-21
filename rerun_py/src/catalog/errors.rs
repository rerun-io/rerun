//! Error handling for the catalog module.
//!
//! ## Guide
//!
//! - For each encountered error (e.g. `re_uri::Error` when parsing), define a new variant in the
//!   `Error` enum, and implement a mapping to a user-facing Python error in the `to_py_err`
//!   function. Then, use `?`.
//!
//! - For errors at the API boundaries between client and server, prefer mapping to
//!   [`re_redap_client::ApiError`] rather than wrapping the error in a new enum variant.
//!
//! - Don't hesitate to introduce new error classes if this could help the user catch specific
//!   errors. Use the [`pyo3::create_exception`] macro for that and update [`super::register`] to
//!   expose it.
//!
//! - Error type (either built-in such as [`pyo3::exceptions::PyValueError`] or custom) can always
//!   be used directly using, e.g. `PyValueError::new_err("message")`.

use pyo3::PyErr;
use pyo3::exceptions::{
    PyConnectionError, PyException, PyPermissionError, PyRuntimeError, PyTimeoutError, PyValueError,
};
use re_redap_client::{ApiErrorKind, TonicStatusError};

pyo3::create_exception!(
    rerun_bindings.rerun_bindings,
    NotFoundError,
    PyException,
    "Raised when the requested resource is not found."
);

pyo3::create_exception!(
    rerun_bindings.rerun_bindings,
    AlreadyExistsError,
    PyException,
    "Raised when trying to create a resource that already exists."
);

// ---

/// Private error type to server as a bridge between various external error type and the
/// [`to_py_err`] function.
#[derive(Debug, thiserror::Error)]
#[expect(clippy::enum_variant_names)] // this is by design
enum ExternalError {
    #[error("{0}")]
    TonicStatusError(Box<TonicStatusError>),

    #[error("{0}")]
    TonicTransportError(Box<tonic::transport::Error>),

    #[error("{0}")]
    UriError(#[from] re_uri::Error),

    #[error("{0}")]
    ChunkError(Box<re_chunk::ChunkError>),

    #[error("{0}")]
    ChunkStoreError(Box<re_chunk_store::ChunkStoreError>),

    #[error("{0}")]
    ApiError(Box<re_redap_client::ApiError>),

    #[error("{0}")]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("{0}")]
    UrlParseError(#[from] url::ParseError),

    #[error(transparent)]
    CodecError(#[from] re_log_encoding::rrd::CodecError),

    #[error(transparent)]
    SorbetError(#[from] re_sorbet::SorbetError),

    #[error(transparent)]
    ColumnSelectorParseError(#[from] re_sorbet::ColumnSelectorParseError),

    #[error(transparent)]
    ColumnSelectorResolveError(#[from] re_sorbet::ColumnSelectorResolveError),

    #[error(transparent)]
    TypeConversionError(Box<re_protos::TypeConversionError>),

    #[error(transparent)]
    TokenError(#[from] re_auth::TokenError),
}

const _: () = assert!(
    std::mem::size_of::<ExternalError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

macro_rules! impl_from_boxed {
    ($external_type:ty, $variant:ident) => {
        impl From<$external_type> for ExternalError {
            fn from(value: $external_type) -> Self {
                Self::$variant(Box::new(value))
            }
        }
    };
}

impl_from_boxed!(re_chunk::ChunkError, ChunkError);
impl_from_boxed!(re_chunk_store::ChunkStoreError, ChunkStoreError);
impl_from_boxed!(re_redap_client::ApiError, ApiError);
impl_from_boxed!(tonic::transport::Error, TonicTransportError);
impl_from_boxed!(re_redap_client::TonicStatusError, TonicStatusError);

impl From<tonic::Status> for ExternalError {
    fn from(value: tonic::Status) -> Self {
        Self::TonicStatusError(Box::new(TonicStatusError::from(value)))
    }
}

/// Classify a [`DataFusionError`] into the closest [`ApiErrorKind`]
fn apierror_kind_for_df_error(err: &datafusion::error::DataFusionError) -> ApiErrorKind {
    use datafusion::error::DataFusionError as D;
    match err {
        // User-input / query-authoring errors.
        D::Plan(_) | D::SchemaError(_, _) | D::SQL(_, _) | D::Configuration(_) => {
            ApiErrorKind::InvalidArguments
        }
        D::NotImplemented(_) => ApiErrorKind::Unimplemented,
        D::ResourcesExhausted(_) => ApiErrorKind::ResourcesExhausted,
        // Everything else — including `DataFusionError::Execution` (DataFusion's
        // grab-bag for invariant failures and data-shape issues, which is also
        // what our own `exec_err!` uses)
        _ => ApiErrorKind::Internal,
    }
}

impl From<datafusion::error::DataFusionError> for ExternalError {
    fn from(value: datafusion::error::DataFusionError) -> Self {
        // Via `re_datafusion::errors`, `DataFusionError::External`
        // can wrap an `ApiError`. Walk the source chain; if we find one,
        // surface it directly. Otherwise synthesize a typed ApiError with a kind
        // inferred from the DataFusionError variant
        if let Some(api) = re_error::downcast_source::<re_redap_client::ApiError>(&value) {
            return Self::ApiError(Box::new(api.clone()));
        }
        let kind = apierror_kind_for_df_error(&value);
        let message = match kind {
            ApiErrorKind::InvalidArguments => "DataFusion query error",
            ApiErrorKind::Unimplemented => "DataFusion feature not implemented",
            ApiErrorKind::ResourcesExhausted => "DataFusion resources exhausted",
            _ => "DataFusion error",
        };
        Self::ApiError(Box::new(re_redap_client::ApiError::with_kind_and_source(
            kind, None, value, message,
        )))
    }
}

impl_from_boxed!(re_protos::TypeConversionError, TypeConversionError);

impl From<ExternalError> for PyErr {
    fn from(err: ExternalError) -> Self {
        match err {
            ExternalError::TonicStatusError(err) => {
                let status: &tonic::Status = (*err).as_ref();
                if status.code() == tonic::Code::DeadlineExceeded {
                    PyTimeoutError::new_err("Deadline expired before operation could complete")
                } else {
                    PyConnectionError::new_err(err.to_string())
                }
            }

            ExternalError::TonicTransportError(err) => PyConnectionError::new_err(err.to_string()),

            ExternalError::UriError(err) => PyValueError::new_err(format!("Invalid URI: {err}")),

            ExternalError::ChunkError(err) => PyValueError::new_err(format!("Chunk error: {err}")),

            ExternalError::ChunkStoreError(err) => {
                PyValueError::new_err(format!("Chunk store error: {err}"))
            }

            ExternalError::ApiError(err) => match err.kind {
                ApiErrorKind::Connection
                | ApiErrorKind::InvalidServer
                | ApiErrorKind::ResourcesExhausted => PyConnectionError::new_err(err.to_string()),
                ApiErrorKind::Unauthenticated | ApiErrorKind::PermissionDenied => {
                    PyPermissionError::new_err(err.to_string())
                }
                ApiErrorKind::Deserialization
                | ApiErrorKind::Serialization
                | ApiErrorKind::InvalidArguments => PyValueError::new_err(err.to_string()),
                ApiErrorKind::NotFound => NotFoundError::new_err(err.to_string()),
                ApiErrorKind::AlreadyExists => AlreadyExistsError::new_err(err.to_string()),
                ApiErrorKind::Timeout => PyTimeoutError::new_err(err.to_string()),
                ApiErrorKind::Unimplemented | ApiErrorKind::Internal => {
                    PyRuntimeError::new_err(err.to_string())
                }
            },

            ExternalError::ArrowError(err) => PyValueError::new_err(format!("Arrow error: {err}")),

            ExternalError::UrlParseError(err) => {
                PyValueError::new_err(format!("Could not parse URL: {err}"))
            }

            ExternalError::CodecError(err) => PyValueError::new_err(format!("Codec error: {err}")),

            ExternalError::SorbetError(err) => {
                PyValueError::new_err(format!("Sorbet error: {err}"))
            }

            ExternalError::ColumnSelectorParseError(err) => PyValueError::new_err(format!("{err}")),

            ExternalError::ColumnSelectorResolveError(err) => {
                PyValueError::new_err(format!("{err}"))
            }

            ExternalError::TypeConversionError(err) => {
                PyValueError::new_err(format!("Could not convert gRPC message: {err}"))
            }

            ExternalError::TokenError(err) => {
                PyPermissionError::new_err(format!("Invalid token: {err}"))
            }
        }
    }
}

/// Global mapping of all our internal error to user-facing Python errors.
///
/// Use as `.map_err(to_py_err)?`.
#[expect(private_bounds)] // this is by design
pub fn to_py_err(err: impl Into<ExternalError>) -> PyErr {
    err.into().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::error::DataFusionError;
    use re_redap_client::{ApiError, ApiErrorKind, TraceId};

    /// A `DataFusionError::External` wrapping an `ApiError` must be recovered
    /// as `ExternalError::ApiError`, with the trace-id and kind preserved.
    #[test]
    fn recovers_embedded_api_error_with_trace_id() {
        let trace_id =
            TraceId::from_hex("0123456789abcdef0123456789abcdef").expect("valid trace-id");
        let arrow_err = arrow::error::ArrowError::SchemaError("rerun schema mismatch: boom".into());
        let api = ApiError::with_kind_and_source(
            ApiErrorKind::Internal,
            Some(trace_id),
            arrow_err,
            "DataFusion schema mismatch error",
        );
        let df_err = DataFusionError::External(Box::new(api));

        let external: ExternalError = df_err.into();
        let ExternalError::ApiError(recovered) = external else {
            panic!("expected ExternalError::ApiError, got something else");
        };
        assert_eq!(recovered.kind, ApiErrorKind::Internal);
        let display = recovered.to_string();
        assert!(
            display.contains("DataFusion schema mismatch error"),
            "message missing: {display}"
        );
        assert!(
            display.contains(&trace_id.to_string()),
            "trace-id missing: {display}"
        );
    }

    /// `DataFusionError::Context` wraps another DataFusionError with a string.
    /// The source-chain walk should still find an embedded `ApiError` through
    /// the Context wrapper.
    #[test]
    fn recovers_api_error_through_context_wrapper() {
        let api = ApiError::with_kind_and_source(
            ApiErrorKind::NotFound,
            None,
            arrow::error::ArrowError::SchemaError("inner".into()),
            "dataset not found",
        );
        let df_err = DataFusionError::Context(
            "while scanning".into(),
            Box::new(DataFusionError::External(Box::new(api))),
        );

        let external: ExternalError = df_err.into();
        let ExternalError::ApiError(recovered) = external else {
            panic!("expected ExternalError::ApiError");
        };
        assert_eq!(recovered.kind, ApiErrorKind::NotFound);
    }
}
