//! The rerun package is made up of 2 pieces
//!
//! [`sdk`] - is set of API calls for logging data. This is used by the assorted rerun language bindings.
//! [`app`] - is the runtime application used for inspecting the data. This is used by the rerun main binary.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

pub mod app;
pub mod sdk;
