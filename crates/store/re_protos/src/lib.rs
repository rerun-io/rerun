//! This crate contains generated types for the remote store gRPC service API.
//! Generation is done using the `re_protos_builder` crate.
//!
//! We want clear separation between 'internal' types and gRPC types and don't want
//! to use gRPC types in the rerun viewer codebase. That's why we implement all the
//! necessary conversion code (in the form of `From` and `TryFrom` traits) in this crate.

pub mod external {
    pub use prost;
}

pub mod headers;

// This extra module is needed, because of how imports from different packages are resolved.
// For example, `rerun.remote_store.v1alpha1.EncoderVersion` is resolved to `super::super::remote_store::v1alpha1::EncoderVersion`.
// We need an extra module in the path to `common` to make that work.
// Additionally, the `common` module itself has to exist with a `v1alpha1` module inside of it,
// which is the reason for the `common`, `log_msg`, `remote_store`, etc. modules below.

// Note: Be careful with `#[path]` attributes: https://github.com/rust-lang/rust/issues/35016
mod v1alpha1 {
    // Note: `allow(clippy::all)` does NOT allow all lints
    #![expect(
        clippy::all,
        clippy::allow_attributes,
        clippy::nursery,
        clippy::pedantic
    )]

    #[path = "./rerun.common.v1alpha1.rs"]
    pub mod rerun_common_v1alpha1;

    #[path = "./rerun.common.v1alpha1.ext.rs"]
    pub mod rerun_common_v1alpha1_ext;

    #[path = "./rerun.log_msg.v1alpha1.rs"]
    pub mod rerun_log_msg_v1alpha1;

    #[path = "./rerun.log_msg.v1alpha1.ext.rs"]
    pub mod rerun_log_msg_v1alpha1_ext;

    #[path = "./rerun.sdk_comms.v1alpha1.rs"]
    pub mod rerun_sdk_comms_v1alpha1;

    #[path = "./rerun.cloud.v1alpha1.rs"]
    pub mod rerun_cloud_v1alpha1;

    #[path = "./rerun.cloud.v1alpha1.ext.rs"]
    pub mod rerun_cloud_v1alpha1_ext;
}

pub mod common {
    pub mod v1alpha1 {
        pub use crate::v1alpha1::rerun_common_v1alpha1::*;
        pub mod ext {
            pub use crate::v1alpha1::rerun_common_v1alpha1_ext::*;
        }
    }
}

pub mod log_msg {
    pub mod v1alpha1 {
        pub use crate::v1alpha1::rerun_log_msg_v1alpha1::*;
    }
}

pub mod cloud {
    pub mod v1alpha1 {
        pub use crate::v1alpha1::rerun_cloud_v1alpha1::*;
        pub mod ext {
            pub use crate::v1alpha1::rerun_cloud_v1alpha1_ext::*;
        }
    }
}

pub mod sdk_comms {
    pub mod v1alpha1 {
        pub use crate::v1alpha1::rerun_sdk_comms_v1alpha1::*;
    }
}

// ---

#[derive(Debug, thiserror::Error)]
pub enum TypeConversionError {
    #[error("missing required field: '{package_name}.{type_name}.{field_name}'")]
    MissingField {
        package_name: &'static str,
        type_name: &'static str,
        field_name: &'static str,
    },

    #[error("invalid value for field '{package_name}.{type_name}.{field_name}: {reason}'")]
    InvalidField {
        package_name: &'static str,
        type_name: &'static str,
        field_name: &'static str,
        reason: String,
    },

    #[error("missing required dataframe column {column_name:?} in '{package_name}.{type_name}'")]
    MissingColumn {
        package_name: &'static str,
        type_name: &'static str,
        column_name: &'static str,
    },

    #[error("invalid dataframe schema in '{package_name}.{type_name}'")]
    InvalidSchema {
        package_name: &'static str,
        type_name: &'static str,
    },

    #[error("failed to parse timestamp: {0}")]
    InvalidTime(#[from] jiff::Error),

    #[error("failed to decode: {0}")]
    DecodeError(#[from] prost::DecodeError),

    #[error("failed to encode: {0}")]
    EncodeError(#[from] prost::EncodeError),

    #[error("failed to convert arrow data: {0}")]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("{0}")]
    UnknownEnumValue(#[from] prost::UnknownEnumValue),

    #[error("could not parse url: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("internal error: {0}")]
    InternalError(String),

    //TODO(#10730): delete when removing 0.24 back compat
    #[error("unexpected legacy `StoreId`: {0}")]
    LegacyStoreIdError(String),
}

impl TypeConversionError {
    #[inline]
    pub fn missing_field<T: prost::Name>(field_name: &'static str) -> Self {
        Self::MissingField {
            package_name: T::PACKAGE,
            type_name: T::NAME,
            field_name,
        }
    }

    #[inline]
    pub fn invalid_field<T: prost::Name>(field_name: &'static str, reason: &impl ToString) -> Self {
        Self::InvalidField {
            package_name: T::PACKAGE,
            type_name: T::NAME,
            field_name,
            reason: reason.to_string(),
        }
    }

    #[inline]
    pub fn invalid_schema<T: prost::Name>() -> Self {
        Self::InvalidSchema {
            package_name: T::PACKAGE,
            type_name: T::NAME,
        }
    }

    #[inline]
    pub fn missing_column<T: prost::Name>(column_name: &'static str) -> Self {
        Self::MissingColumn {
            package_name: T::PACKAGE,
            type_name: T::NAME,
            column_name,
        }
    }
}

impl From<TypeConversionError> for tonic::Status {
    #[inline]
    fn from(value: TypeConversionError) -> Self {
        Self::invalid_argument(value.to_string())
    }
}

#[cfg(feature = "py")]
impl From<TypeConversionError> for pyo3::PyErr {
    #[inline]
    fn from(value: TypeConversionError) -> Self {
        pyo3::exceptions::PyValueError::new_err(value.to_string())
    }
}

/// Create [`TypeConversionError::MissingField`]
#[macro_export]
macro_rules! missing_field {
    ($type:ty, $field:expr $(,)?) => {
        $crate::TypeConversionError::missing_field::<$type>($field)
    };
}

/// Create [`TypeConversionError::InvalidField`]
#[macro_export]
macro_rules! invalid_field {
    ($type:ty, $field:expr, $reason:expr $(,)?) => {
        $crate::TypeConversionError::invalid_field::<$type>($field, &$reason)
    };
}

/// Create [`TypeConversionError::InvalidSchema`]
#[macro_export]
macro_rules! invalid_schema {
    ($type:ty $(,)?) => {
        $crate::TypeConversionError::invalid_schema::<$type>()
    };
}

/// Create [`TypeConversionError::MissingColumn`]
#[macro_export]
macro_rules! missing_column {
    ($type:ty, $column:expr $(,)?) => {
        $crate::TypeConversionError::missing_column::<$type>($column)
    };
}

// ---

// TODO(cmc): move this somewhere else
mod sizes {
    use re_byte_size::SizeBytes;

    impl SizeBytes for crate::log_msg::v1alpha1::LogMsg {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { msg } = self;

            match msg {
                Some(msg) => msg.heap_size_bytes(),
                None => 0,
            }
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::log_msg::Msg {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            match self {
                Self::SetStoreInfo(set_store_info) => set_store_info.heap_size_bytes(),
                Self::ArrowMsg(arrow_msg) => arrow_msg.heap_size_bytes(),
                Self::BlueprintActivationCommand(blueprint_activation_command) => {
                    blueprint_activation_command.heap_size_bytes()
                }
            }
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::SetStoreInfo {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { row_id, info } = self;

            row_id.heap_size_bytes() + info.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v1alpha1::Tuid {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { inc, time_ns } = self;

            inc.heap_size_bytes() + time_ns.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::StoreInfo {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            #[expect(deprecated)]
            let Self {
                application_id: _,
                store_id,
                store_source,
                store_version,
            } = self;

            store_id.heap_size_bytes()
                + store_source.heap_size_bytes()
                + store_version.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v1alpha1::ApplicationId {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { id } = self;

            id.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v1alpha1::StoreId {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                kind,
                recording_id,
                application_id,
            } = self;

            kind.heap_size_bytes()
                + recording_id.heap_size_bytes()
                + application_id.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v1alpha1::TableId {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { id } = self;

            id.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::StoreSource {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { kind, extra } = self;

            kind.heap_size_bytes() + extra.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::StoreSourceExtra {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { payload } = self;

            payload.len() as _
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::StoreVersion {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { crate_version_bits } = self;

            crate_version_bits.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::ArrowMsg {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                store_id,
                chunk_id,
                compression,
                uncompressed_size,
                encoding,
                payload,
                is_static: _,
            } = self;

            store_id.heap_size_bytes()
                + chunk_id.heap_size_bytes()
                + compression.heap_size_bytes()
                + uncompressed_size.heap_size_bytes()
                + encoding.heap_size_bytes()
                + payload.len() as u64
        }
    }

    impl SizeBytes for crate::log_msg::v1alpha1::BlueprintActivationCommand {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                blueprint_id,
                make_active,
                make_default,
            } = self;

            blueprint_id.heap_size_bytes()
                + make_active.heap_size_bytes()
                + make_default.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v1alpha1::DataframePart {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                encoder_version,
                payload,
                compression: _,
                uncompressed_size: _,
            } = self;

            encoder_version.heap_size_bytes()
                + payload.as_ref().map_or(0, |payload| payload.len() as u64)
        }
    }
}
