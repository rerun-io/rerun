//! The different types that make up the rerun log format.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!
//! ## Mono-components
//!
//! Some components, mostly transform related ones, are "mono-components".
//! This means that Rerun makes assumptions that depend on this component
//! only taking on a singular value for all instances of an Entity. Where possible,
//! exposed APIs will force these components to be logged as a singular instance
//! or a splat. However, it is an error with undefined behavior to manually use lower-level
//! APIs to log a batched mono-component.
//!
//! This requirement is especially apparent with transforms:
//! Each entity must have a unique transform chain,
//! e.g. the entity `foo/bar/baz` is has the transform that is the product of
//! `foo.transform * foo/bar.transform * foo/bar/baz.transform`.

pub mod arrow_msg;
mod component;
mod data_cell;
mod data_row;
mod data_table;
pub mod hash;
mod index;
mod instance_key;
pub mod path;
mod size_bytes;
mod time;
pub mod time_point;
mod time_range;
mod time_real;

#[cfg(not(target_arch = "wasm32"))]
mod data_table_batcher;

#[cfg(feature = "serde")]
pub mod serde_field;

use std::sync::Arc;

pub use self::arrow_msg::ArrowMsg;
pub use self::component::{Component, DeserializableComponent, SerializableComponent};
pub use self::data_cell::{DataCell, DataCellError, DataCellInner, DataCellResult};
pub use self::data_row::{DataRow, DataRowError, DataRowResult, RowId};
pub use self::data_table::{
    DataCellColumn, DataCellOptVec, DataTable, DataTableError, DataTableResult, EntityPathVec,
    ErasedTimeVec, NumInstancesVec, RowIdVec, TableId, TimePointVec, COLUMN_ENTITY_PATH,
    COLUMN_INSERT_ID, COLUMN_NUM_INSTANCES, COLUMN_ROW_ID, COLUMN_TIMEPOINT, METADATA_KIND,
    METADATA_KIND_CONTROL, METADATA_KIND_DATA,
};
pub use self::index::*;
pub use self::instance_key::InstanceKey;
pub use self::path::*;
pub use self::size_bytes::SizeBytes;
pub use self::time::{Duration, Time};
pub use self::time_point::{TimeInt, TimePoint, TimeType, Timeline, TimelineName};
pub use self::time_range::{TimeRange, TimeRangeF};
pub use self::time_real::TimeReal;

#[cfg(not(target_arch = "wasm32"))]
pub use self::data_table_batcher::{
    DataTableBatcher, DataTableBatcherConfig, DataTableBatcherError,
};

pub mod external {
    pub use arrow2;
    pub use arrow2_convert;
    pub use re_tuid;
}

#[macro_export]
macro_rules! impl_into_enum {
    ($from_ty: ty, $enum_name: ident, $to_enum_variant: ident) => {
        impl From<$from_ty> for $enum_name {
            #[inline]
            fn from(value: $from_ty) -> Self {
                Self::$to_enum_variant(value)
            }
        }
    };
}

// ----------------------------------------------------------------------------

/// What type of `Recording` this is.
///
/// `Data` recordings contain user-data logged via `log_` API calls.
///
/// In the future, `Blueprint` recordings describe how that data is laid out
/// in the viewer, though this is not currently supported.
///
/// Both of these types can go over the same stream and be stored in the
/// same datastore, but the viewer wants to treat them very differently.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RecordingType {
    /// A recording of user-data.
    Data,

    /// Not currently used: recording data associated with the blueprint state.
    Blueprint,
}

impl std::fmt::Display for RecordingType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data => "Data".fmt(f),
            Self::Blueprint => "Blueprint".fmt(f),
        }
    }
}

/// A unique id per recording (a stream of [`LogMsg`]es).
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RecordingId {
    pub variant: RecordingType,
    pub id: Arc<String>,
}

impl RecordingId {
    #[inline]
    pub fn random(variant: RecordingType) -> Self {
        Self {
            variant,
            id: Arc::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    #[inline]
    pub fn from_uuid(variant: RecordingType, uuid: uuid::Uuid) -> Self {
        Self {
            variant,
            id: Arc::new(uuid.to_string()),
        }
    }

    #[inline]
    pub fn from_string(variant: RecordingType, str: String) -> Self {
        Self {
            variant,
            id: Arc::new(str),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }
}

impl std::fmt::Display for RecordingId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { variant, id } = self;
        f.write_fmt(format_args!("{variant}:{id}"))?;
        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// The user-chosen name of the application doing the logging.
///
/// Used to categorize recordings.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ApplicationId(pub String);

impl From<&str> for ApplicationId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for ApplicationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl ApplicationId {
    /// The default [`ApplicationId`] if the user hasn't set one.
    ///
    /// Currently: `"unknown_app_id"`.
    pub fn unknown() -> Self {
        Self("unknown_app_id".to_owned())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ApplicationId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ----------------------------------------------------------------------------

/// The most general log message sent from the SDK to the server.
#[must_use]
#[derive(Clone, Debug, PartialEq)] // `PartialEq` used for tests in another crate
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[allow(clippy::large_enum_variant)]
pub enum LogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    SetRecordingInfo(SetRecordingInfo),

    /// Server-backed operation on an [`EntityPath`].
    EntityPathOpMsg(RecordingId, EntityPathOpMsg),

    /// Log an entity using an [`ArrowMsg`].
    ArrowMsg(RecordingId, ArrowMsg),
}

impl LogMsg {
    pub fn recording_id(&self) -> &RecordingId {
        match self {
            Self::SetRecordingInfo(msg) => &msg.info.recording_id,
            Self::EntityPathOpMsg(recording_id, _) | Self::ArrowMsg(recording_id, _) => {
                recording_id
            }
        }
    }
}

impl_into_enum!(SetRecordingInfo, LogMsg, SetRecordingInfo);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SetRecordingInfo {
    pub row_id: RowId,
    pub info: RecordingInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RecordingInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: ApplicationId,

    /// Should be unique for each recording.
    pub recording_id: RecordingId,

    /// True if the recording is one of the official Rerun examples.
    pub is_official_example: bool,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub started: Time,

    pub recording_source: RecordingSource,

    pub recording_type: RecordingType,
}

impl RecordingInfo {
    /// Whether this `RecordingInfo` is the default used when a user is not explicitly
    /// creating their own blueprint.
    pub fn is_app_default_blueprint(&self) -> bool {
        self.application_id.as_str() == self.recording_id.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PythonVersion {
    /// e.g. 3
    pub major: u8,

    /// e.g. 11
    pub minor: u8,

    /// e.g. 0
    pub patch: u8,

    /// e.g. `a0` for alpha releases.
    pub suffix: String,
}

impl std::fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            major,
            minor,
            patch,
            suffix,
        } = self;
        write!(f, "{major}.{minor}.{patch}{suffix}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RecordingSource {
    Unknown,

    /// The official Rerun Python Logging SDK
    PythonSdk(PythonVersion),

    /// The official Rerun Rust Logging SDK
    RustSdk {
        rustc_version: String,
        llvm_version: String,
    },

    /// Perhaps from some manual data ingestion?
    Other(String),
}

impl std::fmt::Display for RecordingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => "Unknown".fmt(f),
            Self::PythonSdk(version) => write!(f, "Python {version} SDK"),
            Self::RustSdk {
                rustc_version: rust_version,
                llvm_version: _,
            } => write!(f, "Rust {rust_version} SDK"),
            Self::Other(string) => format!("{string:?}").fmt(f), // put it in quotes
        }
    }
}

// ----------------------------------------------------------------------------

/// An operation (like a 'clear') on an [`EntityPath`].
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPathOpMsg {
    /// A unique id per [`EntityPathOpMsg`].
    pub row_id: RowId,

    /// Time information (when it was logged, when it was received, …).
    ///
    /// If this is empty, no operation will be performed as we
    /// cannot be timeless in a meaningful way.
    pub time_point: TimePoint,

    /// What operation.
    pub path_op: PathOp,
}

/// Operation to perform on an [`EntityPath`], e.g. clearing all components.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PathOp {
    /// Clear all the components stored at an [`EntityPath`]
    ClearComponents(EntityPath),

    /// Clear all the components of an `[EntityPath]` and any descendants.
    ClearRecursive(EntityPath),
}

impl PathOp {
    pub fn clear(recursive: bool, entity_path: EntityPath) -> Self {
        if recursive {
            PathOp::ClearRecursive(entity_path)
        } else {
            PathOp::ClearComponents(entity_path)
        }
    }

    pub fn entity_path(&self) -> &EntityPath {
        match &self {
            PathOp::ClearComponents(path) | PathOp::ClearRecursive(path) => path,
        }
    }
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
