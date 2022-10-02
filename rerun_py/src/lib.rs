//! The Rerun Python Log SDK.
//!
//! This provides bindings between Python and Rust.
//! It compiles into a Python wheel using <https://github.com/PyO3/pyo3>.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod python_bridge;
pub(crate) mod sdk;
