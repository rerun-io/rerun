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
pub mod example_components;
pub mod hash;
mod index;
pub mod path;
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
pub use self::component::LegacyComponent;
pub use self::data_cell::{DataCell, DataCellError, DataCellInner, DataCellResult};
pub use self::data_row::{
    DataCellRow, DataCellVec, DataReadError, DataReadResult, DataRow, DataRowError, DataRowResult,
    RowId,
};
pub use self::data_table::{
    DataCellColumn, DataCellOptVec, DataTable, DataTableError, DataTableResult, EntityPathVec,
    ErasedTimeVec, NumInstancesVec, RowIdVec, TableId, TimePointVec, COLUMN_ENTITY_PATH,
    COLUMN_INSERT_ID, COLUMN_NUM_INSTANCES, COLUMN_ROW_ID, COLUMN_TIMEPOINT, METADATA_KIND,
    METADATA_KIND_CONTROL, METADATA_KIND_DATA,
};
pub use self::index::*;
pub use self::path::*;
pub use self::time::{Duration, Time};
pub use self::time_point::{TimeInt, TimePoint, TimeType, Timeline, TimelineName};
pub use self::time_range::{TimeRange, TimeRangeF};
pub use self::time_real::TimeReal;

// Re-export `ComponentName` for convenience
pub use re_types::ComponentName;
pub use re_types::SizeBytes;

#[cfg(not(target_arch = "wasm32"))]
pub use self::data_table_batcher::{
    DataTableBatcher, DataTableBatcherConfig, DataTableBatcherError,
};

mod load_file;

#[cfg(not(target_arch = "wasm32"))]
pub use self::load_file::data_cells_from_file_path;

pub use self::load_file::{data_cells_from_file_contents, FromFileError};

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

/// What kind of Store this is.
///
/// `Recording` stores contain user-data logged via `log_` API calls.
///
/// In the future, `Blueprint` stores describe how that data is laid out
/// in the viewer, though this is not currently supported.
///
/// Both of these kinds can go over the same stream and be stored in the
/// same datastore, but the viewer wants to treat them very differently.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum StoreKind {
    /// A recording of user-data.
    Recording,

    /// Data associated with the blueprint state.
    Blueprint,
}

impl std::fmt::Display for StoreKind {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording => "Recording".fmt(f),
            Self::Blueprint => "Blueprint".fmt(f),
        }
    }
}

/// A unique id per store.
///
/// The kind of store is part of the id, and can be either a
/// [`StoreKind::Recording`] or a [`StoreKind::Blueprint`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct StoreId {
    pub kind: StoreKind,
    pub id: Arc<String>,
}

impl StoreId {
    #[inline]
    pub fn random(kind: StoreKind) -> Self {
        Self {
            kind,
            id: Arc::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    #[inline]
    pub fn from_uuid(kind: StoreKind, uuid: uuid::Uuid) -> Self {
        Self {
            kind,
            id: Arc::new(uuid.to_string()),
        }
    }

    #[inline]
    pub fn from_string(kind: StoreKind, str: String) -> Self {
        Self {
            kind,
            id: Arc::new(str),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }
}

impl std::fmt::Display for StoreId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `StoreKind` is not part of how we display the id,
        // because that can easily lead to confusion and bugs
        // when roundtripping to a string (e.g. via Python SDK).
        self.id.fmt(f)
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
    SetStoreInfo(SetStoreInfo),

    /// Server-backed operation on an [`EntityPath`].
    EntityPathOpMsg(StoreId, EntityPathOpMsg),

    /// Log an entity using an [`ArrowMsg`].
    ArrowMsg(StoreId, ArrowMsg),
}

impl LogMsg {
    pub fn store_id(&self) -> &StoreId {
        match self {
            Self::SetStoreInfo(msg) => &msg.info.store_id,
            Self::EntityPathOpMsg(store_id, _) | Self::ArrowMsg(store_id, _) => store_id,
        }
    }
}

impl_into_enum!(SetStoreInfo, LogMsg, SetStoreInfo);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SetStoreInfo {
    pub row_id: RowId,
    pub info: StoreInfo,
}

/// Information about a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct StoreInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: ApplicationId,

    /// Should be unique for each recording.
    pub store_id: StoreId,

    /// True if the recording is one of the official Rerun examples.
    pub is_official_example: bool,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub started: Time,

    pub store_source: StoreSource,

    pub store_kind: StoreKind,
}

impl StoreInfo {
    /// Whether this `StoreInfo` is the default used when a user is not explicitly
    /// creating their own blueprint.
    pub fn is_app_default_blueprint(&self) -> bool {
        self.application_id.as_str() == self.store_id.as_str()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FileSource {
    Cli,
    DragAndDrop,
    FileDialog,
}

/// The source of a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum StoreSource {
    Unknown,

    /// The official Rerun C Logging SDK
    CSdk,

    /// The official Rerun Python Logging SDK
    PythonSdk(PythonVersion),

    /// The official Rerun Rust Logging SDK
    RustSdk {
        /// Rust version of the the code compiling the Rust SDK
        rustc_version: String,

        /// LLVM version of the the code compiling the Rust SDK
        llvm_version: String,
    },

    /// Loading a file via CLI, drag-and-drop, a file-dialog, etc.
    File {
        file_source: FileSource,
    },

    /// Perhaps from some manual data ingestion?
    Other(String),
}

impl std::fmt::Display for StoreSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => "Unknown".fmt(f),
            Self::CSdk => "C SDK".fmt(f),
            Self::PythonSdk(version) => write!(f, "Python {version} SDK"),
            Self::RustSdk { rustc_version, .. } => write!(f, "Rust SDK (rustc {rustc_version})"),
            Self::File { file_source, .. } => match file_source {
                FileSource::Cli => write!(f, "File via CLI"),
                FileSource::DragAndDrop => write!(f, "File via drag-and-drop"),
                FileSource::FileDialog => write!(f, "File via file dialog"),
            },
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

#[doc(hidden)]
#[macro_export]
macro_rules! component_legacy_shim {
    ($entity:ident) => {

        impl re_types::Loggable for $entity {
            type Name = re_types::ComponentName;

            #[inline]
            fn name() -> Self::Name {
                <Self as re_log_types::LegacyComponent>::legacy_name()
                    .as_str()
                    .into()
            }

            #[inline]
            fn arrow_datatype() -> arrow2::datatypes::DataType {
                <Self as re_log_types::LegacyComponent>::field().data_type
            }

            #[inline]
            fn try_to_arrow_opt<'a>(
                data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
            ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
            where
                Self: Clone + 'a,
            {
                let input = data.into_iter().map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum.map(|d| d.into_owned())
                });

                let vec: Vec<_> = input.collect();

                let arrow = arrow2_convert::serialize::TryIntoArrow::try_into_arrow(vec.iter())
                    .map_err(|err| {
                        re_types::SerializationError::ArrowConvertFailure(err.to_string())
                    })?;

                Ok(arrow)
            }

            #[inline]
            fn try_from_arrow_opt(data: &dyn ::arrow2::array::Array) -> re_types::DeserializationResult<Vec<Option<Self>>>
            where
                Self: Sized
            {
                let native = <
                    <Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as arrow2_convert::deserialize::ArrowArray
                >::iter_from_array_ref(data);
                Ok(
                    native
                        .into_iter()
                        .map(|item| <Self as arrow2_convert::deserialize::ArrowDeserialize>::arrow_deserialize(item))
                        .collect()
                )
            }
        }

        impl<'a> From<$entity> for ::std::borrow::Cow<'a, $entity> {
            #[inline]
            fn from(value: $entity) -> Self {
                std::borrow::Cow::Owned(value)
            }
        }

        impl<'a> From<&'a $entity> for ::std::borrow::Cow<'a, $entity> {
            #[inline]
            fn from(value: &'a $entity) -> Self {
                std::borrow::Cow::Borrowed(value)
            }
        }
    };
}

// ---

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`TimePoint`].
#[inline]
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
    (Timeline::log_time(), log_time.into())
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`TimePoint`].
#[inline]
pub fn build_frame_nr(frame_nr: TimeInt) -> (Timeline, TimeInt) {
    (Timeline::new("frame_nr", TimeType::Sequence), frame_nr)
}
