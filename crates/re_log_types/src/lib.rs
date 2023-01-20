//! Types used for the log data.
//!
//! Rerun is based around _objects_ and _data_.
//!
//! Example objects includes points, rectangles, images, … (see [`ObjectType`] for more).
//! Each of these has many _fields_. For instance, a point
//! has fields `pos`, `radius`, `color`, etc.
//!
//! When you log an object, you log each field seperatedly,
//! as [`Data`].
//!
//! Each object is logged to a specific [`ObjPath`] -
//! check out module-level documentation for [`path`] for more on this.
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
pub mod field_types;
pub use arrow_msg::ArrowMsg;
mod data;
pub mod hash;
mod index;
pub mod msg_bundle;
pub mod objects;
pub mod path;
mod time;
pub mod time_point;
mod time_range;
mod time_real;

pub mod external {
    pub use arrow2;
    pub use arrow2_convert;
}

pub use self::data::*;
pub use self::field_types::context;
pub use self::field_types::coordinates;
pub use self::field_types::AnnotationContext;
pub use self::field_types::Arrow3D;
pub use self::field_types::MsgId;
pub use self::field_types::ViewCoordinates;
pub use self::field_types::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use self::index::*;
pub use self::objects::ObjectType;
pub use self::path::*;
pub use self::time::{Duration, Time};
pub use self::time_point::{TimeInt, TimePoint, TimeType, Timeline, TimelineName};
pub use self::time_range::{TimeRange, TimeRangeF};
pub use self::time_real::TimeReal;

pub type ComponentName = FieldName;

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
    /// Should usually be the first message sent.
    BeginRecordingMsg(BeginRecordingMsg),

    /// Log type-info ([`ObjectType`]) to a [`ObjTypePath`].
    TypeMsg(TypeMsg),

    /// Log some data to a [`DataPath`].
    DataMsg(DataMsg),

    /// Server-backed operation on an [`ObjPath`] or [`DataPath`].
    PathOpMsg(PathOpMsg),

    /// Log an arrow message to a [`DataPath`].
    ArrowMsg(ArrowMsg),

    /// Sent when the client shuts down the connection.
    Goodbye(MsgId),
}

impl LogMsg {
    pub fn id(&self) -> MsgId {
        match self {
            Self::BeginRecordingMsg(msg) => msg.msg_id,
            Self::TypeMsg(msg) => msg.msg_id,
            Self::DataMsg(msg) => msg.msg_id,
            Self::PathOpMsg(msg) => msg.msg_id,
            Self::ArrowMsg(msg) => msg.msg_id,
            Self::Goodbye(msg_id) => *msg_id,
        }
    }
}

impl_into_enum!(BeginRecordingMsg, LogMsg, BeginRecordingMsg);
impl_into_enum!(TypeMsg, LogMsg, TypeMsg);
impl_into_enum!(DataMsg, LogMsg, DataMsg);
impl_into_enum!(PathOpMsg, LogMsg, PathOpMsg);
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

    /// Perhaps from som manual data ingestion?
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

/// The message sent to specify the [`ObjectType`] of all objects at a specific [`ObjTypePath`].
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TypeMsg {
    /// A unique id per [`LogMsg`].
    pub msg_id: MsgId,

    /// The [`ObjTypePath`] target.
    pub type_path: ObjTypePath,

    /// The type of object at this object type path.
    pub obj_type: ObjectType,
}

impl TypeMsg {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn obj_type(type_path: ObjTypePath, obj_type: ObjectType) -> Self {
        Self {
            msg_id: MsgId::random(),
            type_path,
            obj_type,
        }
    }
}

// ----------------------------------------------------------------------------

/// The message sent to specify the data of a single field of an object.
#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataMsg {
    /// A unique id per [`DataMsg`].
    pub msg_id: MsgId,

    /// Time information (when it was logged, when it was received, …)
    ///
    /// If this is empty, the data is _timeless_.
    /// Timeless data will show up on all timelines, past and future,
    /// and will hit all time queries. In other words, it is always there.
    pub time_point: TimePoint,

    /// What the data is targeting.
    pub data_path: DataPath,

    /// The value of this.
    pub data: LoggedData,
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum LoggedData {
    /// An empty data value, for "mono-objects", indicates the data is no longer valid
    Null(DataType),

    /// A single data value, for "mono-objects".
    Single(Data),

    /// Log multiple values at once to a "multi-object".
    ///
    /// The index becomes an "instance index" that, together with the object-path, forms an "instance".
    Batch { indices: BatchIndex, data: DataVec },

    /// Log the same value for all instances of a multi-object.
    ///
    /// You can only use this for optional fields such as `color`, `space` etc.
    /// You can NOT use it for primary fields such as `pos`.
    BatchSplat(Data),
}

impl LoggedData {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Null(data_type) => *data_type,
            Self::Single(data) | Self::BatchSplat(data) => data.data_type(),
            Self::Batch { data, .. } => data.element_data_type(),
        }
    }
}

impl From<Data> for LoggedData {
    #[inline]
    fn from(data: Data) -> Self {
        Self::Single(data)
    }
}

#[macro_export]
macro_rules! impl_into_logged_data {
    ($from_ty: ty, $data_enum_variant: ident) => {
        impl From<$from_ty> for LoggedData {
            #[inline]
            fn from(value: $from_ty) -> Self {
                Self::Single(Data::$data_enum_variant(value))
            }
        }
    };
}

impl_into_logged_data!(i32, I32);
impl_into_logged_data!(f32, F32);
impl_into_logged_data!(BBox2D, BBox2D);
impl_into_logged_data!(ClassicTensor, Tensor);
impl_into_logged_data!(Box3, Box3);
impl_into_logged_data!(Mesh3D, Mesh3D);
impl_into_logged_data!(ObjPath, ObjPath);

// ----------------------------------------------------------------------------
/// The message sent to specify the data of a single field of an object.
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PathOpMsg {
    /// A unique id per [`PathOpMsg`].
    pub msg_id: MsgId,

    /// Time information (when it was logged, when it was received, …)
    ///
    /// If this is empty, no operation will be performed as ObjPathOps
    /// cannot be Timeless in a meaningful way.
    pub time_point: TimePoint,

    /// The value of this.
    pub path_op: PathOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PathOp {
    // Clear all the fields stored at an [`ObjPath`]
    ClearFields(ObjPath),
    // Clear all the fields of an `[ObjPath]` and any descendents.
    ClearRecursive(ObjPath),
}

impl PathOp {
    pub fn clear(recursive: bool, obj_path: ObjPath) -> Self {
        if recursive {
            PathOp::ClearRecursive(obj_path)
        } else {
            PathOp::ClearFields(obj_path)
        }
    }

    pub fn obj_path(&self) -> &ObjPath {
        match &self {
            PathOp::ClearFields(path) | PathOp::ClearRecursive(path) => path,
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
