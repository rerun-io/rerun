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

pub mod context;
pub mod coordinates;
mod data;
pub mod hash;
mod index;
pub mod objects;
pub mod path;
mod time;

pub use context::AnnotationContext;
pub use coordinates::ViewCoordinates;
pub use data::*;
pub use index::*;
pub use objects::ObjectType;
pub use path::*;
pub use time::{Duration, Time};

use std::collections::BTreeMap;

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

/// A unique id per [`LogMsg`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MsgId(pub uuid::Uuid);

impl nohash_hasher::IsEnabled for MsgId {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for MsgId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl MsgId {
    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
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
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
}

impl LogMsg {
    pub fn id(&self) -> MsgId {
        match self {
            Self::BeginRecordingMsg(msg) => msg.msg_id,
            Self::TypeMsg(msg) => msg.msg_id,
            Self::DataMsg(msg) => msg.msg_id,
            Self::PathOpMsg(msg) => msg.msg_id,
        }
    }
}

impl_into_enum!(BeginRecordingMsg, LogMsg, BeginRecordingMsg);
impl_into_enum!(TypeMsg, LogMsg, TypeMsg);
impl_into_enum!(DataMsg, LogMsg, DataMsg);
impl_into_enum!(PathOpMsg, LogMsg, PathOpMsg);

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

#[inline]
pub fn data_msg(
    time_point: &TimePoint,
    obj_path: impl Into<ObjPath>,
    field_name: impl Into<FieldName>,
    data: impl Into<LoggedData>,
) -> DataMsg {
    DataMsg {
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.into(), field_name.into()),
        data: data.into(),
        msg_id: MsgId::random(),
    }
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
impl_into_logged_data!(Tensor, Tensor);
impl_into_logged_data!(Box3, Box3);
impl_into_logged_data!(Mesh3D, Mesh3D);
impl_into_logged_data!(ObjPath, ObjPath);

// ----------------------------------------------------------------------------
/// The message sent to specify the data of a single field of an object.
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PathOpMsg {
    /// A unique id per [`ObjPathOpMsg`].
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
    // Clear a single field at a [`DataPath`]
    ClearField(DataPath),
    // Clear all the fields stored at an [`ObjPath`]
    ClearFields(ObjPath),
    // Clear all the fields of an `[ObjPath]` and any descendents.
    ClearRecursive(ObjPath),
}

// ----------------------------------------------------------------------------

re_string_interner::declare_new_type!(
    /// The name of a timeline. Often something like `"log_time"` or `"frame_nr"`.
    pub struct TimelineName;
);

impl Default for TimelineName {
    fn default() -> Self {
        Self::new("")
    }
}

// ----------------------------------------------------------------------------

/// A time frame/space, e.g. `log_time` or `frame_nr`, coupled with the type of time
/// it keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timeline {
    /// Name of the timeline (e.g. "log_time").
    name: TimelineName,

    /// Sequence or time?
    typ: TimeType,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            name: TimelineName::new(""),
            typ: TimeType::Sequence,
        }
    }
}

impl Timeline {
    #[inline]
    pub fn new(name: impl Into<TimelineName>, typ: TimeType) -> Self {
        Self {
            name: name.into(),
            typ,
        }
    }

    #[inline]
    pub fn name(&self) -> &TimelineName {
        &self.name
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }
}

impl nohash_hasher::IsEnabled for Timeline {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for Timeline {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.name.hash() | self.typ.hash());
    }
}

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
///
/// If this is empty, the data is _timeless_.
/// Timeless data will show up on all timelines, past and future,
/// and will hit all time queries. In other words, it is always there.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<Timeline, TimeInt>);

impl TimePoint {
    /// Logging to this time means the data will show upp in all timelines,
    /// past and future. The time will be [`TimeInt::BEGINNING`], meaning it will
    /// always be in range for any time query.
    pub fn timeless() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.0.is_empty()
    }
}

// ----------------------------------------------------------------------------

/// The type of a [`TimeInt`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

impl TimeType {
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }

    pub fn format(&self, time_int: TimeInt) -> String {
        if time_int <= TimeInt::BEGINNING {
            "-∞".into()
        } else {
            match self {
                Self::Time => Time::from(time_int).format(),
                Self::Sequence => format!("#{}", time_int.0),
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Either nanoseconds or sequence numbers.
///
/// Must be matched with a [`TimeType`] to know what.
///
/// Used both for time points and durations.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeInt(i64);

impl TimeInt {
    /// The beginning of time.
    ///
    /// Special value used for timeless data.
    ///
    /// NOTE: this is not necessarily [`i64::MIN`].
    // The reason we don't use i64::MIN is because in the time panel we need
    // to be able to pan to before the `TimeInt::BEGINNING`, and so we need
    // a bit of leeway.
    pub const BEGINNING: TimeInt = TimeInt(i64::MIN / 2);

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn as_f32(&self) -> f32 {
        self.0 as _
    }

    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0 as _
    }

    #[inline]
    pub fn abs(&self) -> Self {
        Self(self.0.saturating_abs())
    }
}

impl From<i64> for TimeInt {
    #[inline]
    fn from(seq: i64) -> Self {
        Self(seq)
    }
}

impl From<Duration> for TimeInt {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self(duration.as_nanos())
    }
}

impl From<Time> for TimeInt {
    #[inline]
    fn from(time: Time) -> Self {
        Self(time.nanos_since_epoch())
    }
}

impl From<TimeInt> for Time {
    fn from(int: TimeInt) -> Self {
        Self::from_ns_since_epoch(int.as_i64())
    }
}

impl From<TimeInt> for Duration {
    fn from(int: TimeInt) -> Self {
        Self::from_nanos(int.as_i64())
    }
}

impl std::ops::Neg for TimeInt {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
}

impl std::ops::Add for TimeInt {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for TimeInt {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::AddAssign for TimeInt {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign for TimeInt {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::iter::Sum for TimeInt {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = TimeInt(0);
        for item in iter {
            sum += item;
        }
        sum
    }
}

// ----------------------------------------------------------------------------

#[inline]
pub fn time_point(
    fields: impl IntoIterator<Item = (&'static str, TimeType, TimeInt)>,
) -> TimePoint {
    TimePoint(
        fields
            .into_iter()
            .map(|(name, tt, ti)| (Timeline::new(name, tt), ti))
            .collect(),
    )
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_scope!($($arg)*);
    };
}
