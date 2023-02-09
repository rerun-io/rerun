//! Types used for the log data.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![allow(clippy::manual_range_contains)]

#[cfg(any(feature = "save", feature = "load"))]
pub mod encoding;

#[cfg(feature = "arrow_datagen")]
pub mod datagen;

pub mod arrow_msg;
pub mod component_types;
pub use arrow_msg::ArrowMsg;
mod data;
pub mod hash;
mod index;
pub mod msg_bundle;
pub mod path;
mod time;
pub mod time_point;
mod time_range;
mod time_real;

pub mod external {
    pub use arrow2;
    pub use arrow2_convert;

    #[cfg(feature = "glam")]
    pub use glam;
}

pub use self::component_types::context;
pub use self::component_types::coordinates;
pub use self::component_types::AnnotationContext;
pub use self::component_types::Arrow3D;
pub use self::component_types::MsgId;
pub use self::component_types::ViewCoordinates;
pub use self::component_types::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use self::data::*;
pub use self::index::*;
pub use self::path::*;
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ApplicationId(pub String);

impl ApplicationId {
    pub fn unknown() -> Self {
        Self("unknown_app_id".to_owned())
    }
}

// ----------------------------------------------------------------------------

/// The most general log message sent from the SDK to the server.
#[must_use]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(test, derive(PartialEq))]
#[allow(clippy::large_enum_variant)]
pub enum LogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    BeginRecordingMsg(BeginRecordingMsg),

    /// Server-backed operation on an [`EntityPath`].
    EntityPathOpMsg(EntityPathOpMsg),

    /// Log an entity using an [`ArrowMsg`].
    ArrowMsg(ArrowMsg),

    /// Sent when the client shuts down the connection.
    Goodbye(MsgId),
}

impl LogMsg {
    pub fn id(&self) -> MsgId {
        match self {
            Self::BeginRecordingMsg(msg) => msg.msg_id,
            Self::EntityPathOpMsg(msg) => msg.msg_id,
            Self::ArrowMsg(msg) => msg.msg_id,
            Self::Goodbye(msg_id) => *msg_id,
        }
    }
}

impl_into_enum!(BeginRecordingMsg, LogMsg, BeginRecordingMsg);
impl_into_enum!(EntityPathOpMsg, LogMsg, EntityPathOpMsg);
impl_into_enum!(ArrowMsg, LogMsg, ArrowMsg);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BeginRecordingMsg {
    pub msg_id: MsgId,

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
pub enum RecordingSource {
    /// The official Rerun Python Logging SDK
    PythonSdk,

    /// Perhaps from some manual data ingestion?
    Other(String),
}

impl std::fmt::Display for RecordingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PythonSdk => "Python SDK".fmt(f),
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
    pub msg_id: MsgId,

    /// Time information (when it was logged, when it was received, …).
    ///
    /// If this is empty, no operation will be performed as we
    /// cannot be timeless in a meaningful way.
    pub time_point: TimePoint,

    /// What operation.
    pub path_op: PathOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PathOp {
    // Clear all the components stored at an [`EntityPath`]
    ClearComponents(EntityPath),

    // Clear all the components of an `[EntityPath]` and any descendants.
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
