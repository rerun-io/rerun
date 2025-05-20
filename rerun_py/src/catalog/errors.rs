//! Error handling for the catalog module.
//!
//! ## Guide
//!
//! - For each encountered error (e.g. `re_uri::Error` when parsing), define a new variant in the
//!   `Error` enum, and implement a mapping to a user-facing Python error in the `to_py_err`
//!   function. Then, use `?`.
//!
//! - Don't hesitate to introduce new error classes if this could help the user catch specific
//!   errors. Use the [`pyo3::create_exception`] macro for that and update [`super::register`] to
//!   expose it.
//!
//! - Error type (either built-in such as [`pyo3::exceptions::PyValueError`] or custom) can always
//!   be used directly using, e.g. `PyValueError::new_err("message")`.

use std::error::Error as _;

use pyo3::PyErr;
use pyo3::exceptions::{PyConnectionError, PyTimeoutError, PyValueError};

use re_grpc_client::redap::ConnectionError;
use re_protos::manifest_registry::v1alpha1::ext::GetDatasetSchemaResponseError;

// ---

/// Private error type to server as a bridge between various external error type and the
/// [`to_py_err`] function.
#[derive(Debug, thiserror::Error)]
#[expect(clippy::enum_variant_names)] // this is by design
enum ExternalError {
    #[error("{0}")]
    ConnectionError(#[from] ConnectionError),

    #[error("{0}")]
    TonicStatusError(#[from] tonic::Status),

    #[error("{0}")]
    UriError(#[from] re_uri::Error),

    #[error("{0}")]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error("{0}")]
    ChunkStoreError(#[from] re_chunk_store::ChunkStoreError),

    #[error("{0}")]
    StreamError(#[from] re_grpc_client::StreamError),

    #[error("{0}")]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("{0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("{0}")]
    DatafusionError(#[from] datafusion::error::DataFusionError),

    #[error(transparent)]
    CodecError(#[from] re_log_encoding::codec::CodecError),

    #[error(transparent)]
    SorbetError(#[from] re_sorbet::SorbetError),

    #[error(transparent)]
    ColumnSelectorParseError(#[from] re_sorbet::ColumnSelectorParseError),

    #[error(transparent)]
    ColumnSelectorResolveError(#[from] re_sorbet::ColumnSelectorResolveError),

    #[error(transparent)]
    TypeConversionError(#[from] re_protos::TypeConversionError),
}

impl From<re_protos::manifest_registry::v1alpha1::ext::GetDatasetSchemaResponseError>
    for ExternalError
{
    fn from(value: GetDatasetSchemaResponseError) -> Self {
        match value {
            GetDatasetSchemaResponseError::ArrowError(err) => err.into(),
            GetDatasetSchemaResponseError::TypeConversionError(err) => {
                re_grpc_client::StreamError::from(err).into()
            }
        }
    }
}

impl From<ExternalError> for PyErr {
    fn from(err: ExternalError) -> Self {
        match err {
            ExternalError::ConnectionError(err) => PyConnectionError::new_err(err.to_string()),

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

            ExternalError::UriError(err) => PyValueError::new_err(format!("Invalid URI: {err}")),

            ExternalError::ChunkError(err) => PyValueError::new_err(format!("Chunk error: {err}")),

            ExternalError::ChunkStoreError(err) => {
                PyValueError::new_err(format!("Chunk store error: {err}"))
            }

            ExternalError::StreamError(err) => {
                PyValueError::new_err(format!("Data streaming error: {err}"))
            }

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
