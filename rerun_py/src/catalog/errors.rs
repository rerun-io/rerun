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

use std::error::Error as _;

use pyo3::PyErr;
use pyo3::exceptions::{
    PyConnectionError, PyException, PyPermissionError, PyRuntimeError, PyTimeoutError, PyValueError,
};
use re_redap_client::ApiErrorKind;

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
    TonicStatusError(Box<tonic::Status>),

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

    #[error("{0}")]
    DatafusionError(Box<datafusion::error::DataFusionError>),

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
impl_from_boxed!(tonic::Status, TonicStatusError);
impl_from_boxed!(datafusion::error::DataFusionError, DatafusionError);
impl_from_boxed!(re_protos::TypeConversionError, TypeConversionError);

impl From<ExternalError> for PyErr {
    fn from(err: ExternalError) -> Self {
        match err {
            ExternalError::TonicStatusError(status) => {
                if status.code() == tonic::Code::DeadlineExceeded {
                    PyTimeoutError::new_err("Deadline expired before operation could complete")
                } else {
                    let mut msg = format!(
                        "tonic status error: {} (code: {}",
                        status.message(),
                        status.code()
                    );
                    if let Some(source) = status.source() {
                        msg.push_str(&format!(", source: {source})"));
                    } else {
                        msg.push(')');
                    }

                    PyConnectionError::new_err(msg)
                }
            }

            ExternalError::TonicTransportError(err) => PyConnectionError::new_err(err.to_string()),

            ExternalError::UriError(err) => PyValueError::new_err(format!("Invalid URI: {err}")),

            ExternalError::ChunkError(err) => PyValueError::new_err(format!("Chunk error: {err}")),

            ExternalError::ChunkStoreError(err) => {
                PyValueError::new_err(format!("Chunk store error: {err}"))
            }

            ExternalError::ApiError(err) => match err.kind {
                ApiErrorKind::Connection | ApiErrorKind::InvalidServer => {
                    PyConnectionError::new_err(err.to_string())
                }
                ApiErrorKind::Unauthenticated | ApiErrorKind::PermissionDenied => {
                    PyPermissionError::new_err(err.to_string())
                }
                ApiErrorKind::Serialization | ApiErrorKind::InvalidArguments => {
                    PyValueError::new_err(err.to_string())
                }
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

            ExternalError::DatafusionError(err) => {
                PyValueError::new_err(format!("DataFusion error: {err}"))
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
