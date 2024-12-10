//! This crate contains generated types for the remote store gRPC service API.
//! Generation is done using the `re_protos_builder` crate.
//!
//! We want clear separation between 'internal' types and gRPC types and don't want
//! to use gRPC types in the rerun viewer codebase. That's why we implement all the
//! necessary conversion code (in the form of `From` and `TryFrom` traits) in this crate.
//!

pub mod external {
    pub use prost;
}

// This extra module is needed, because of how imports from different packages are resolved.
// For example, `rerun.remote_store.v0.EncoderVersion` is resolved to `super::super::remote_store::v0::EncoderVersion`.
// We need an extra module in the path to `common` to make that work.
// Additionally, the `common` module itself has to exist with a `v0` module inside of it,
// which is the reason for the `common`, `log_msg`, `remote_store`, etc. modules below.

// Note: Be careful with `#[path]` attributes: https://github.com/rust-lang/rust/issues/35016
mod v0 {
    // Note: `allow(clippy::all)` does NOT allow all lints
    #![allow(clippy::all, clippy::pedantic, clippy::nursery)]

    #[path = "./rerun.common.v0.rs"]
    pub mod rerun_common_v0;

    #[path = "./rerun.log_msg.v0.rs"]
    pub mod rerun_log_msg_v0;

    #[path = "./rerun.remote_store.v0.rs"]
    pub mod rerun_remote_store_v0;
}

pub mod common {
    pub mod v0 {
        pub use crate::v0::rerun_common_v0::*;
    }
}

pub mod log_msg {
    pub mod v0 {
        pub use crate::v0::rerun_log_msg_v0::*;
    }
}

/// Generated types for the remote store gRPC service API v0.
pub mod remote_store {
    pub mod v0 {
        pub use crate::v0::rerun_remote_store_v0::*;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TypeConversionError {
    #[error("missing required field: {type_name}.{field_name}")]
    MissingField {
        type_name: &'static str,
        field_name: &'static str,
    },

    #[error("invalid value for field {type_name}.{field_name}: {reason}")]
    InvalidField {
        type_name: &'static str,
        field_name: &'static str,
        reason: String,
    },

    #[error("failed to decode: {0}")]
    DecodeError(#[from] prost::DecodeError),

    #[error("failed to encode: {0}")]
    EncodeError(#[from] prost::EncodeError),

    #[error("{0}")]
    UnknownEnumValue(#[from] prost::UnknownEnumValue),
}

impl TypeConversionError {
    pub fn missing_field(type_name: &'static str, field_name: &'static str) -> Self {
        Self::MissingField {
            type_name,
            field_name,
        }
    }
}
