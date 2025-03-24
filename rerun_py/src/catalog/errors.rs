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

use pyo3::exceptions::{PyConnectionError, PyValueError};
use pyo3::{create_exception, PyErr};

use re_grpc_client::redap::ConnectionError;

// ---

// Custom exception classes.
create_exception!(catalog, MissingGrpcFieldError, PyValueError);

// ---

/// Private error type to server as a bridge between various external error type and the
/// [`to_py_err`] function.
#[expect(clippy::enum_variant_names)] // this is by design
enum ExternalError {
    ConnectionError(ConnectionError),
    TonicStatusError(tonic::Status),
    UriError(re_uri::Error),
}

impl From<ConnectionError> for ExternalError {
    fn from(value: ConnectionError) -> Self {
        Self::ConnectionError(value)
    }
}

impl From<tonic::Status> for ExternalError {
    fn from(value: tonic::Status) -> Self {
        Self::TonicStatusError(value)
    }
}

impl From<re_uri::Error> for ExternalError {
    fn from(value: re_uri::Error) -> Self {
        Self::UriError(value)
    }
}

impl From<ExternalError> for PyErr {
    fn from(err: ExternalError) -> Self {
        match err {
            ExternalError::ConnectionError(err) => PyConnectionError::new_err(err.to_string()),

            ExternalError::TonicStatusError(status) => {
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

            ExternalError::UriError(err) => PyValueError::new_err(format!("Invalid URI: {err}")),
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
