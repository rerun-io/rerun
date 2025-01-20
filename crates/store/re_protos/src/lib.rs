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

    #[path = "./rerun.sdk_comms.v0.rs"]
    pub mod rerun_sdk_comms_v0;
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

        /// Recording catalog mandatory field names. All mandatory metadata fields are prefixed
        /// with "rerun_" to avoid conflicts with user-defined fields.
        pub const CATALOG_ID_FIELD_NAME: &str = "rerun_recording_id";
        pub const CATALOG_APP_ID_FIELD_NAME: &str = "rerun_application_id";
        pub const CATALOG_START_TIME_FIELD_NAME: &str = "rerun_start_time";
        pub const CATALOG_DESCRIPTION_FIELD_NAME: &str = "rerun_description";
        pub const CATALOG_RECORDING_TYPE_FIELD_NAME: &str = "rerun_recording_type";
        pub const CATALOG_STORAGE_URL_FIELD_NAME: &str = "rerun_storage_url";
        pub const CATALOG_REGISTRATION_TIME_FIELD_NAME: &str = "rerun_registration_time";
        pub const CATALOG_ROW_ID_FIELD_NAME: &str = "rerun_row_id";
    }
}

pub mod sdk_comms {
    pub mod v0 {
        pub use crate::v0::rerun_sdk_comms_v0::*;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TypeConversionError {
    #[error("missing required field: {package_name}.{type_name}.{field_name}")]
    MissingField {
        package_name: &'static str,
        type_name: &'static str,
        field_name: &'static str,
    },

    #[error("invalid value for field {package_name}.{type_name}.{field_name}: {reason}")]
    InvalidField {
        package_name: &'static str,
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
    #[inline]
    pub fn missing_field<T: prost::Name>(field_name: &'static str) -> Self {
        Self::MissingField {
            package_name: T::PACKAGE,
            type_name: T::NAME,
            field_name,
        }
    }

    #[allow(clippy::needless_pass_by_value)] // false-positive
    #[inline]
    pub fn invalid_field<T: prost::Name>(field_name: &'static str, reason: &impl ToString) -> Self {
        Self::InvalidField {
            package_name: T::PACKAGE,
            type_name: T::NAME,
            field_name,
            reason: reason.to_string(),
        }
    }
}

#[macro_export]
macro_rules! missing_field {
    ($type:ty, $field:expr $(,)?) => {
        $crate::TypeConversionError::missing_field::<$type>($field)
    };
}

#[macro_export]
macro_rules! invalid_field {
    ($type:ty, $field:expr, $reason:expr $(,)?) => {
        $crate::TypeConversionError::invalid_field::<$type>($field, &$reason)
    };
}

mod sizes {
    use re_byte_size::SizeBytes;

    impl SizeBytes for crate::log_msg::v0::LogMsg {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { msg } = self;

            match msg {
                Some(msg) => msg.heap_size_bytes(),
                None => 0,
            }
        }
    }

    impl SizeBytes for crate::log_msg::v0::log_msg::Msg {
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

    impl SizeBytes for crate::log_msg::v0::SetStoreInfo {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { row_id, info } = self;

            row_id.heap_size_bytes() + info.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v0::Tuid {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { inc, time_ns } = self;

            inc.heap_size_bytes() + time_ns.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::StoreInfo {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                application_id,
                store_id,
                is_official_example,
                started,
                store_source,
                store_version,
            } = self;

            application_id.heap_size_bytes()
                + store_id.heap_size_bytes()
                + is_official_example.heap_size_bytes()
                + started.heap_size_bytes()
                + store_source.heap_size_bytes()
                + store_version.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v0::ApplicationId {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { id } = self;

            id.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v0::StoreId {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { kind, id } = self;

            kind.heap_size_bytes() + id.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::common::v0::Time {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { nanos_since_epoch } = self;

            nanos_since_epoch.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::StoreSource {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { kind, extra } = self;

            kind.heap_size_bytes() + extra.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::StoreSourceExtra {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { payload } = self;

            payload.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::StoreVersion {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self { crate_version_bits } = self;

            crate_version_bits.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::ArrowMsg {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self {
                store_id,
                compression,
                uncompressed_size,
                encoding,
                payload,
            } = self;

            store_id.heap_size_bytes()
                + compression.heap_size_bytes()
                + uncompressed_size.heap_size_bytes()
                + encoding.heap_size_bytes()
                + payload.heap_size_bytes()
        }
    }

    impl SizeBytes for crate::log_msg::v0::BlueprintActivationCommand {
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
}
