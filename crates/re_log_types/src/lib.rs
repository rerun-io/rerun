//! The different types that make up the rerun log format.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "arrow_datagen")]
pub mod datagen;

pub mod arrow_msg;
mod component;
pub mod component_types;
mod data;
mod data_cell;
mod data_row;
mod data_table;
pub mod hash;
mod index;
pub mod path;
mod size_bytes;
mod time;
pub mod time_point;
mod time_range;
mod time_real;

pub mod external {
    pub use arrow2;
    pub use arrow2_convert;
    pub use re_tuid;

    #[cfg(feature = "glam")]
    pub use glam;

    #[cfg(feature = "image")]
    pub use image;
}

pub use self::arrow_msg::ArrowMsg;
pub use self::component::{Component, DeserializableComponent, SerializableComponent};
pub use self::component_types::context;
pub use self::component_types::coordinates;
pub use self::component_types::AnnotationContext;
pub use self::component_types::Arrow3D;
pub use self::component_types::DecodedTensor;
pub use self::component_types::{
    EncodedMesh3D, ImuData, Mesh3D, MeshFormat, MeshId, RawMesh3D, XlinkStats,
};
pub use self::component_types::{Tensor, ViewCoordinates};
pub use self::data::*;
pub use self::data_cell::{DataCell, DataCellError, DataCellInner, DataCellResult};
pub use self::data_row::{DataRow, DataRowError, DataRowResult, RowId};
pub use self::data_table::{
    DataCellColumn, DataCellOptVec, DataTable, DataTableError, DataTableResult, EntityPathVec,
    ErasedTimeVec, NumInstancesVec, RowIdVec, TableId, TimePointVec, COLUMN_ENTITY_PATH,
    COLUMN_INSERT_ID, COLUMN_NUM_INSTANCES, COLUMN_ROW_ID, COLUMN_TIMEPOINT, METADATA_KIND,
    METADATA_KIND_CONTROL, METADATA_KIND_DATA,
};
pub use self::index::*;
pub use self::path::*;
pub use self::size_bytes::SizeBytes;
pub use self::time::{Duration, Time};
pub use self::time_point::{TimeInt, TimePoint, TimeType, Timeline, TimelineName};
pub use self::time_range::{TimeRange, TimeRangeF};
pub use self::time_real::TimeReal;

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

/// A unique id per recording (a stream of [`LogMsg`]es).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RecordingId(uuid::Uuid);

impl nohash_hasher::IsEnabled for RecordingId {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for RecordingId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl Default for RecordingId {
    fn default() -> Self {
        Self::ZERO
    }
}

impl RecordingId {
    /// The recording id:s given to recordings that don't have an ID.
    pub const ZERO: RecordingId = RecordingId(uuid::Uuid::nil());

    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    #[inline]
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

impl std::fmt::Display for RecordingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for RecordingId {
    type Err = <uuid::Uuid as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
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
    BeginRecordingMsg(BeginRecordingMsg),

    /// Server-backed operation on an [`EntityPath`].
    EntityPathOpMsg(RecordingId, EntityPathOpMsg),

    /// Log an entity using an [`ArrowMsg`].
    ArrowMsg(RecordingId, ArrowMsg),

    /// Sent when the client shuts down the connection.
    Goodbye(RowId),
}

impl LogMsg {
    pub fn id(&self) -> RowId {
        match self {
            Self::BeginRecordingMsg(msg) => msg.row_id,
            Self::EntityPathOpMsg(_, msg) => msg.row_id,
            Self::Goodbye(row_id) => *row_id,
            // TODO(#1619): the following only makes sense because, while we support sending and
            // receiving batches, we don't actually do so yet.
            // We need to stop storing raw `LogMsg`s before we can benefit from our batching.
            Self::ArrowMsg(_, msg) => msg.table_id.into_row_id(),
        }
    }

    pub fn recording_id(&self) -> Option<&RecordingId> {
        match self {
            Self::BeginRecordingMsg(msg) => Some(&msg.info.recording_id),
            Self::EntityPathOpMsg(recording_id, _) | Self::ArrowMsg(recording_id, _) => {
                Some(recording_id)
            }
            Self::Goodbye(_) => None,
        }
    }
}

impl_into_enum!(BeginRecordingMsg, LogMsg, BeginRecordingMsg);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BeginRecordingMsg {
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
